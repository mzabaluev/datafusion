**2025-08-24 14:55** Read the assignment, checked out the DataFusion repo. This is all new to me.

**2025-08-24 23:45** Got back to the assignment after an evening out. 💛💙
The task is to implement a function matching regexp on a column in... DataFrame?

**2025-08-25 00:05** Understood the task to mean working on the DataFusion repo and implementing
the extra function there.

**2025-08-25 00:20** Read up on Scalar UDF. Could be a way to add a function, but as the library
user. The task calls for extending DataFusion itself, keep digging.

**2025-08-25 00:25** Found [regexp_match](https://github.com/apache/datafusion/blob/main/datafusion/functions/src/regex/regexpmatch.rs) among DataFusion functions. It's also a Scalar UDF, seems like
the way to go. Would it be cheating to copy and hack this one? To be continued next day...

**2025-08-25 16:20** Opened DataFusion in devcontainer.

**2025-08-25 16:27** Regexp compatibility? Spark uses Java regexp, the go-to Rust crate, `regex`,
may have differences. As there are no easy to grab crates with full compat, will do with the syntax
supported by the available crate.

**2025-08-25 16:54** Type signature for the function?
Keep it simple: oneof of a number of exact combinations like in `regexp_match`, the idx argument
is always `UInt32`, rely on coercions.

**2025-08-25 17:00** Similar to other functions, allow the first arg to be column or constant.

**2025-08-25 17:35** Got through massaging the function's arguments. Other regexp functions use
regexp functions from `arrow-string`, it seems prudent to do the same to work on the array arg.

**2025-08-25 17:45** Nah, `arrow-string` can only get matches on all groups as dynamic list elements in array, it will be poor use of Rust to extract these dynamically. Back to `regex`.

**2025-08-25 17:55** Got to go fetch wife.

**2025-08-25 21:30** Resumed implementation. How to iterate over a string array of
three possible types? Copied accessor API from other regexp functions, made the implementation
generic over a string iterator (retrieving `Option` items, why must everything be weird).

**2025-08-25 22:30** Implemented the function, wired into the dynamic module, wrote some unit
tests for the impl. Needs an example to prove it works.

**2025-08-25 23:45** Workarounds in argument parsing, added workable examples.
