//! The repository guards: tests that hold the code and the docs to the rules
//! in `docs/ENGINEERING.md`. They are one test binary so they share
//! `support`, and each module's own doc says what it enforces.

mod comment_ids;
mod dead_pub;
mod doc_refs;
mod file_size;
mod support;
mod task_ids;
