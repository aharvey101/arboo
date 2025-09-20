use std::sync::{Arc, Mutex};
use std::collections::HashMap;
use chrono::Local;

#[derive(Debug, Clone)]
pub struct TestAssertion {
    pub description: String,
    pub passed: bool,
    pub error_message: Option<String>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TestSuite {
    pub name: String,
    pub assertions: Vec<TestAssertion>,
    pub start_time: chrono::DateTime<chrono::Local>,
    pub end_time: Option<chrono::DateTime<chrono::Local>>,
}

pub struct Reporter {
    suites: Arc<Mutex<HashMap<String, TestSuite>>>,
}

impl Reporter {
    pub fn new() -> Self {
        Self {
            suites: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn start_suite(&self, suite_name: &str) {
        let mut suites = self.suites.lock().unwrap();
        suites.insert(suite_name.to_string(), TestSuite {
            name: suite_name.to_string(),
            assertions: Vec::new(),
            start_time: Local::now(),
            end_time: None,
        });

        println!("\n🧪 {}", suite_name);
    }

    pub fn should(&self, suite_name: &str, description: &str) -> AssertionBuilder {
        AssertionBuilder::new(suite_name, description, self.suites.clone())
    }

    pub fn end_suite(&self, suite_name: &str) {
        let mut suites = self.suites.lock().unwrap();
        if let Some(suite) = suites.get_mut(suite_name) {
            suite.end_time = Some(Local::now());

            let passed = suite.assertions.iter().filter(|a| a.passed).count();
            let failed = suite.assertions.iter().filter(|a| !a.passed).count();
            let duration = suite.end_time.unwrap() - suite.start_time;

            if failed == 0 {
                println!("  ✅ {} assertions passed ({:.2}s)", passed, duration.num_milliseconds() as f64 / 1000.0);
            } else {
                println!("  ❌ {} passed, {} failed ({:.2}s)", passed, failed, duration.num_milliseconds() as f64 / 1000.0);
            }
        }
    }

    pub fn print_summary(&self) {
        let suites = self.suites.lock().unwrap();
        let mut total_passed = 0;
        let mut total_failed = 0;
        let mut total_suites = 0;
        let mut failed_suites = 0;

        println!("\n📊 Test Summary");
        println!("================");

        for (suite_name, suite) in suites.iter() {
            let passed = suite.assertions.iter().filter(|a| a.passed).count();
            let failed = suite.assertions.iter().filter(|a| !a.passed).count();

            total_passed += passed;
            total_failed += failed;
            total_suites += 1;

            if failed > 0 {
                failed_suites += 1;
                println!("❌ {}", suite_name);
                for assertion in &suite.assertions {
                    if !assertion.passed {
                        println!("    ✗ {}", assertion.description);
                        if let Some(ref error) = assertion.error_message {
                            println!("      {}", error);
                        }
                    }
                }
            } else {
                println!("✅ {}", suite_name);
            }
        }

        println!("\n📈 Results:");
        println!("  {} test suites: {} passed, {} failed", total_suites, total_suites - failed_suites, failed_suites);
        println!("  {} assertions: {} passed, {} failed", total_passed + total_failed, total_passed, total_failed);

        if total_failed == 0 {
            println!("🎉 All tests passed!");
        } else {
            println!("❌ {} test(s) failed", total_failed);
        }
    }
}

pub struct AssertionBuilder {
    suite_name: String,
    description: String,
    suites: Arc<Mutex<HashMap<String, TestSuite>>>,
    start_time: std::time::Instant,
}

impl AssertionBuilder {
    fn new(suite_name: &str, description: &str, suites: Arc<Mutex<HashMap<String, TestSuite>>>) -> Self {
        Self {
            suite_name: suite_name.to_string(),
            description: description.to_string(),
            suites,
            start_time: std::time::Instant::now(),
        }
    }

    pub fn assert<F>(&self, test_fn: F) -> anyhow::Result<()> 
    where 
        F: FnOnce() -> anyhow::Result<()>,
    {
        let result = test_fn();
        let duration = self.start_time.elapsed().as_millis() as u64;

        let assertion = match &result {
            Ok(_) => {
                println!("    ✓ {} ({} ms)", self.description, duration);
                TestAssertion {
                    description: self.description.clone(),
                    passed: true,
                    error_message: None,
                    duration_ms: Some(duration),
                }
            }
            Err(e) => {
                println!("    ✗ {} ({} ms)", self.description, duration);
                println!("      {}", e);
                TestAssertion {
                    description: self.description.clone(),
                    passed: false,
                    error_message: Some(e.to_string()),
                    duration_ms: Some(duration),
                }
            }
        };

        let mut suites = self.suites.lock().unwrap();
        if let Some(suite) = suites.get_mut(&self.suite_name) {
            suite.assertions.push(assertion);
        }

        result
    }

    pub async fn assert_async<F, Fut>(&self, test_fn: F) -> anyhow::Result<()>
    where
        F: FnOnce() -> Fut,
        Fut: std::future::Future<Output = anyhow::Result<()>>,
    {
        let result = test_fn().await;
        let duration = self.start_time.elapsed().as_millis() as u64;

        let assertion = match &result {
            Ok(_) => {
                println!("    ✓ {} ({} ms)", self.description, duration);
                TestAssertion {
                    description: self.description.clone(),
                    passed: true,
                    error_message: None,
                    duration_ms: Some(duration),
                }
            }
            Err(e) => {
                println!("    ✗ {} ({} ms)", self.description, duration);
                println!("      {}", e);
                TestAssertion {
                    description: self.description.clone(),
                    passed: false,
                    error_message: Some(e.to_string()),
                    duration_ms: Some(duration),
                }
            }
        };

        let mut suites = self.suites.lock().unwrap();
        if let Some(suite) = suites.get_mut(&self.suite_name) {
            suite.assertions.push(assertion);
        }

        result
    }
}

