## "Heartbeat" Liveness Monitor

This is a process monitor adapted from [William Murray's](https://github.com/gitxandert/c-from-scratch/commits?author=williamofai]) [c-from-scratch](https://github.com/williamofai/c-from-scratch) course on building safety-critical systems in C. I've translated his code in the [pulse](https://github.com/williamofai/c-from-scratch/tree/main/projects/pulse) directory to idiomatic Rust, adding some Rusty features to the author's logic and tweaking the tests a little to force a data race (hard to do in Rust!).

This is probably too literal of a translation, but I'm mainly doing this as a more constructive way to interact with the author's ideas, rather than reading and copying the code exactly as provided. I could also implement his windowing idea, once I've finished the remainder of the course.
