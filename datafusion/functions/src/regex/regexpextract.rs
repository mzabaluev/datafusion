// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.

//! Regex expressions

use super::compile_regex;
use arrow::array::{StringArray, Array, ArrayRef, AsArray};
use arrow::datatypes::DataType;
use datafusion_common::{exec_err, plan_err};
use datafusion_common::{ScalarValue, Result};
use datafusion_expr::{ColumnarValue, Documentation, TypeSignature};
use datafusion_expr::{ScalarUDFImpl, Signature, Volatility};
use datafusion_macros::user_doc;

use std::any::Any;
use std::sync::Arc;

#[user_doc(
    doc_section(label = "Regular Expression Functions"),
    description = "Extract a specific group matched by the [regular expression](https://docs.rs/regex/latest/regex/#syntax), from the specified string column. If the regex did not match, or the specified group did not match, an empty string is returned.",
    syntax_example = "regexp_extract(name, pattern, idx)",
    sql_example = r#"```sql
            > select regexp_extract(values, ('[a-zA-Z]ö[a-zA-Z]{2}'), 0);
            +---------------------------------------------------------+
            | regexp_extract(examples.values,Utf8("[a-zA-Z]ö[a-zA-Z]{2}", 0)) |
            +---------------------------------------------------------+
            | [Köln]                                                  |
            +---------------------------------------------------------+
```
Additional examples can be found [here](https://github.com/apache/datafusion/blob/main/datafusion-examples/examples/regexp.rs)
"#,
    argument(
        name = "name",
        description = "Target string to work on.
            Can be a constant, column, or function."
    ),
    argument(
        name = "pattern",
        description = "Regular expression to match against.
            Can be a constant, column, or function."
    ),
    argument(
        name = "idx",
        description = "Matched group id."
    )
)]
#[derive(Debug, PartialEq, Eq, Hash)]
pub struct RegexpExtractFunc {
    signature: Signature,
}

impl Default for RegexpExtractFunc {
    fn default() -> Self {
        Self::new()
    }
}

impl RegexpExtractFunc {
    pub fn new() -> Self {
        use DataType::*;
        Self {
            signature: Signature::one_of(
                vec![
                    // Planner attempts coercion to the target type starting with the most preferred candidate.
                    TypeSignature::Exact(vec![Utf8View, Utf8, UInt32]),
                    TypeSignature::Exact(vec![Utf8, Utf8, UInt32]),
                    TypeSignature::Exact(vec![LargeUtf8, Utf8, UInt32]),
                ],
                Volatility::Immutable,
            ),
        }
    }
}

impl ScalarUDFImpl for RegexpExtractFunc {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn name(&self) -> &str {
        "regexp_extract"
    }

    fn signature(&self) -> &Signature {
        &self.signature
    }

    fn return_type(&self, arg_types: &[DataType]) -> Result<DataType> {
        Ok(match &arg_types[0] {
            DataType::Null => DataType::Null,
            _ => DataType::Utf8,
        })
    }

    fn invoke_with_args(
        &self,
        args: datafusion_expr::ScalarFunctionArgs,
    ) -> Result<ColumnarValue> {
        let args = args.args;
        let len = args.len();
        if len != 3 {
            return exec_err!(
                "regexp_extract was called with {len} arguments. It requires 3."
            );
        }
        let mut args = args.into_iter();
        // If the argument is a scalar, convert to array and remember we did.
        let (source, is_scalar) = match args.next().unwrap() {
            ColumnarValue::Scalar(arg) => (arg.to_array()?, true),
            ColumnarValue::Array(arg) => (arg, false),
        };
        let pattern = match args.next().unwrap() {
            ColumnarValue::Scalar(ScalarValue::Utf8(Some(s))) => s,
            ColumnarValue::Scalar(scalar) => return exec_err!(
                "unexpected pattern argument {scalar:?} for regexp_extract"
            ),
            ColumnarValue::Array(_) => return exec_err!(
                "the pattern argument in regexp_extract is expected to be a scalar"
            ),
        };
        let idx = match args.next().unwrap() {
            ColumnarValue::Scalar(ScalarValue::UInt32(Some(i))) => i as usize,
            ColumnarValue::Scalar(v) => return exec_err!(
                "unexpected index argument {v:?} for regexp_extract"
            ),
            ColumnarValue::Array(_) => return exec_err!(
                "the index argument in regexp_extract is expected to be a scalar"
            ),
        };

        let result = match source.data_type() {
            DataType::Utf8View => {
                regexp_extract(source.as_string_view().iter(), &pattern, idx)
            }
            DataType::Utf8 => {
                regexp_extract(source.as_string::<i32>().iter(), &pattern, idx)
            }
            DataType::LargeUtf8 => {
                regexp_extract(source.as_string::<i64>().iter(), &pattern, idx)
            }
            e => {
                return plan_err!("regexp_extract was called with unexpected data type {e:?}");
            }
        };

        if is_scalar {
            // If the input is scalar, keeps output as scalar
            let result = result.and_then(|arr| ScalarValue::try_from_array(&arr, 0));
            result.map(ColumnarValue::Scalar)
        } else {
            result.map(ColumnarValue::Array)
        }
    }

    fn documentation(&self) -> Option<&Documentation> {
        self.doc()
    }
}

fn regexp_extract<'a>(values: impl Iterator<Item = Option<&'a str>>, pattern: &str, idx: usize) -> Result<ArrayRef> {
    let re = compile_regex(pattern, None)?;
    let mut extracts = Vec::new();
    for v in values {
        if let Some(s) = v {
            if let Some(caps) = re.captures(s) {
                if let Some(cap) = caps.get(idx) {
                    extracts.push(cap.as_str());
                    // This is the only success case. In all other outcomes,
                    // fall through to adding an empty string.
                    continue;
                }
            }
        }
        extracts.push("");
    }

    let array = StringArray::from(extracts);
    Ok(Arc::new(array) as ArrayRef)
}

#[cfg(test)]
mod tests {
    use crate::regex::regexpextract::regexp_extract;
    use arrow::array::StringArray;
    use arrow::array::{StringBuilder};

    #[test]
    fn test_regexp_extract_capture_group() {
        let values = StringArray::from(vec!["abc", "def"]);
        let pattern = "^(a)b";

        let mut expected_builder = StringBuilder::new();
        expected_builder.append_value("a");
        expected_builder.append_value("");
        let expected = expected_builder.finish();

        let re = regexp_extract(values.iter(), pattern, 1).unwrap();

        assert_eq!(re.as_ref(), &expected);
    }

    #[test]
    fn test_regexp_extract_whole_match() {
        let values = StringArray::from(vec!["abc", "def"]);
        let pattern = "^a(b)";

        let mut expected_builder = StringBuilder::new();
        expected_builder.append_value("ab");
        expected_builder.append_value("");
        let expected = expected_builder.finish();

        let re = regexp_extract(values.iter(), pattern, 0).unwrap();

        assert_eq!(re.as_ref(), &expected);
    }

    #[test]
    fn test_regexp_extract_no_such_group() {
        let values = StringArray::from(vec!["abc", "def"]);
        let pattern = "^(a)b";

        let mut expected_builder = StringBuilder::new();
        expected_builder.append_value("");
        expected_builder.append_value("");
        let expected = expected_builder.finish();

        let re = regexp_extract(values.iter(), pattern, 2).unwrap();

        assert_eq!(re.as_ref(), &expected);
    }

}
