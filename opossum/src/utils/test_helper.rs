//! various helper functions used to simplify unit tests.
//!
//! **Note**: This module is only compiled and used during testing. Hence, there might be no
//! further documentation show up.

#[cfg(test)]
pub mod test_helper {

    pub fn check_logs(level: log::Level, expected_warnings: Vec<&str>) {
        testing_logger::validate(|captured_logs| {
            let captured_logs: Vec<_> = captured_logs.iter().filter(|l| l.level == level).collect();
            assert_eq!(captured_logs.len(), expected_warnings.len());
            for log in captured_logs.iter().zip(expected_warnings.clone()) {
                assert_eq!(log.0.body, log.1);
            }
        });
    }
}
