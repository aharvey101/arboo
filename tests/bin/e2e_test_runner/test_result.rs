#[derive(Debug)]
pub struct TestResult {
    pub name: String,
    pub success: bool,
    pub error: Option<String>,
}

impl TestResult {
    pub fn success(name: &str) -> Self {
        Self {
            name: name.to_string(),
            success: true,
            error: None,
        }
    }
    
    pub fn failure(name: &str, error: String) -> Self {
        Self {
            name: name.to_string(),
            success: false,
            error: Some(error),
        }
    }
}

pub struct TestResults {
    pub results: Vec<TestResult>,
}

impl TestResults {
    pub fn new() -> Self {
        Self {
            results: Vec::new(),
        }
    }
    
    pub fn add(&mut self, result: TestResult) {
        self.results.push(result);
    }
    
    pub fn has_failures(&self) -> bool {
        self.results.iter().any(|r| !r.success)
    }
    
    pub fn print_summary(&self) {
        println!("\n📊 Test Summary:");
        println!("================");
        
        for result in &self.results {
            if result.success {
                println!("✅ {}", result.name);
            } else {
                println!("❌ {}: {}", result.name, result.error.as_ref().unwrap_or(&"Unknown error".to_string()));
            }
        }
        
        let total = self.results.len();
        let passed = self.results.iter().filter(|r| r.success).count();
        let failed = total - passed;
        
        println!("\n📈 Results: {} passed, {} failed, {} total", passed, failed, total);
        
        if failed > 0 {
            println!("❌ Some tests failed!");
        } else {
            println!("🎉 All tests passed!");
        }
    }
}
