use anyhow::Result;
use config::Config;
use rand::{distributions::Uniform, thread_rng, Rng};
use selenium_manager::get_manager_by_driver;
use serde::Deserialize;
use std::{fs, io::Write, path::PathBuf, process::Child, time::Instant};

use anyhow::anyhow;
use chrono::{DateTime, Local};
use thirtyfour::{prelude::*, ChromeCapabilities};
use tracing::{error, info};

const DOORDASH_URL: &str = "https://identity.doordash.com/auth/user/signup?client_id=1666519390426295040&enable_last_social=false&intl=en-US&layout=consumer_web&prompt=none&redirect_uri=https%3A%2F%2Fwww.doordash.com%2Fpost-login%2F&response_type=code&scope=%2A&state=%2Fen-US%2Fhome%2F%7C%7Cf0e073b3-2117-4d5e-9129-f5254065cdf3";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36";

#[derive(Debug, Deserialize, Default)]
pub struct Configuration {
    first_name: String,
    last_name: String,
    email_name: String,
    email_domain: String,
    address: String,
    #[serde(default = "generate_password")]
    password: String,
    quantity: Option<u32>,
    pub save_to_file: Option<bool>,
    headless: Option<bool>,
    #[serde(default = "default_port")]
    chromedriver_port: i32,
}

#[derive(Default)]
pub struct AccountGenerator {
    pub config: Configuration,
    pub accounts: Vec<Account>,
    caps: ChromeCapabilities,
}

pub struct Account {
    pub email: String,
    pub password: String,
    pub created: DateTime<Local>,
}

impl AccountGenerator {
    pub fn new(config_path: &str, show_output: Option<bool>) -> Result<Self> {
        if show_output.unwrap_or(true) {
            tracing_subscriber::fmt::init();
        }

        let mut self_ = Self {
            ..Default::default()
        };

        self_.config = Self::load_config(config_path)?;
        self_.caps = Self::get_caps(&self_)?;

        Ok(self_)
    }

    #[tokio::main]
    pub async fn run(&mut self) -> Result<()> {
        info!("Starting chromedriver...");
        let driver_path = tokio::task::spawn_blocking(Self::get_driver_path).await??;
        let mut chromedriver = self.run_chromedriver(driver_path)?;

        info!(
            "chromedriver running on port {}",
            &self.config.chromedriver_port
        );

        let quantity = self.config.quantity.unwrap_or(1);

        for i in 0..quantity {
            let start = Instant::now();

            info!("Creating account {} of {}...", i + 1, quantity);

            let driver = WebDriver::new(
                format!("http://localhost:{}", self.config.chromedriver_port).as_str(),
                self.caps.clone(),
            )
            .await?;

            let result = self.automate_signup(&driver).await;

            match result {
                Ok(account) => {
                    info!(
                        "Account generated successfully: {}:{}. Took {:?}s",
                        account.email,
                        account.password,
                        start.elapsed().as_secs_f32()
                    );

                    self.accounts.push(account);
                }
                Err(err) => error!("Failed to generate account: {}", err),
            };

            driver.quit().await?;
        }

        info!("Killing chromedriver...");
        chromedriver.kill()?;

        Ok(())
    }

    pub fn save_to_file(&self, output_path: Option<&str>) -> Result<String> {
        if self.accounts.is_empty() {
            return Err(anyhow!("No accounts generated!"));
        }

        let file_name = Local::now().format("%d-%m-%Y").to_string();
        let mut path = PathBuf::from(output_path.unwrap_or("./"));
        path.push(file_name);
        path.set_extension("txt");

        let mut file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(path.clone())?;

        for account in &self.accounts {
            writeln!(file, "{}:{}", account.email, account.password)?;
        }

        Ok(path.as_os_str().to_str().unwrap().to_string())
    }

    fn load_config(path: &str) -> Result<Configuration> {
        let config = Config::builder()
            .add_source(config::File::with_name(path))
            .add_source(config::Environment::with_prefix("APP"))
            .build()?
            .try_deserialize::<Configuration>()?;

        Ok(config)
    }

    fn run_chromedriver(&self, driver_path: String) -> Result<Child> {
        let chromedriver = std::process::Command::new(driver_path)
            .arg("--ip=localhost")
            .arg(format!("--port={}", &self.config.chromedriver_port))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        Ok(chromedriver)
    }

    fn get_driver_path() -> Result<String> {
        info!("Grabbing chromedriver...");
        let driver_name: String = "chromedriver".to_string();

        let mut selenium_manager = get_manager_by_driver(driver_name).unwrap();

        let path = selenium_manager.resolve_driver().unwrap();

        Ok(path.as_os_str().to_str().unwrap().to_string())
    }

    fn get_caps(&self) -> Result<ChromeCapabilities> {
        let mut caps = DesiredCapabilities::chrome();

        caps.add_chrome_arg("--window-size=1920,1080")?;
        caps.add_chrome_arg("--start-maximized")?;
        caps.add_chrome_arg(format!("--user-agent={}", USER_AGENT).as_str())?;

        if self.config.headless.unwrap_or(true) {
            caps.add_chrome_arg("--headless")?;
        };

        Ok(caps)
    }

    async fn automate_signup(&self, driver: &WebDriver) -> Result<Account> {
        driver.goto(DOORDASH_URL).await?;

        // First name
        driver
            .query(By::Css(
                "input[data-anchor-id=IdentitySignupFirstNameField]",
            ))
            .first()
            .await?
            .send_keys(&self.config.first_name)
            .await?;

        // Last name
        driver
            .query(By::Css("input[data-anchor-id=IdentitySignupLastNameField]"))
            .first()
            .await?
            .send_keys(&self.config.last_name)
            .await?;

        // Email
        let email = format!(
            "{}+{}@{}",
            self.config.email_name,
            thread_rng().gen_range(1000000000..10000000000i64),
            self.config.email_domain
        );

        driver
            .query(By::Css("input[data-anchor-id=IdentitySignupEmailField]"))
            .first()
            .await?
            .send_keys(&email)
            .await?;

        // Country code
        driver
            .query(By::Css("#FieldWrapper-3"))
            .first()
            .await?
            .find(By::XPath("option[@value='AU']"))
            .await?
            .click()
            .await?;

        // Phone number
        let phone_number = format!("0452{}", thread_rng().gen_range(100000..1000000));

        driver
            .query(By::Css("input[data-anchor-id=IdentitySignupPhoneField]"))
            .first()
            .await?
            .send_keys(&phone_number)
            .await?;

        // Password
        driver
            .query(By::Css("input[data-anchor-id=IdentitySignupPasswordField]"))
            .first()
            .await?
            .send_keys(&self.config.password)
            .await?;

        // Submit
        driver
            .query(By::Css("button[data-anchor-id=IdentitySignupSubmitButton]"))
            .first()
            .await?
            .click()
            .await?;

        // Address
        driver
            .query(By::Css("input[aria-label='Your delivery address']"))
            .first()
            .await?
            .send_keys(&self.config.address)
            .await?;

        driver
            .query(By::Css(
                "span[data-anchor-id=AddressAutocompleteSuggestion-0]",
            ))
            .first()
            .await?
            .click()
            .await?;

        Ok(Account {
            email,
            password: self.config.password.clone(),
            created: Local::now(),
        })
    }
}

fn default_port() -> i32 {
    9515
}

fn generate_password() -> String {
    let charset: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ\
                            abcdefghijklmnopqrstuvwxyz\
                            0123456789\
                            !@#$%^&*";
    thread_rng()
        .sample_iter(&Uniform::new_inclusive(0, charset.len() - 1))
        .map(|i| charset[i] as char)
        .take(14)
        .collect()
}
