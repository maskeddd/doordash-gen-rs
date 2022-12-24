use anyhow::Result;
use config::Config;
use rand::{distributions::Uniform, thread_rng, Rng};
use selenium_manager::{get_manager_by_browser, get_manager_by_driver, SeleniumManager};
use serde::Deserialize;
use std::{process::exit, time::Instant};

use thirtyfour::prelude::*;
use tracing::{error, info};

#[derive(Debug, Deserialize, Default)]
struct Configuration {
    first_name: String,
    last_name: String,
    email_name: String,
    email_domain: String,
    address: String,
    #[serde(default = "generate_password")]
    password: String,
    quantity: Option<i32>,
    headless: Option<bool>,
    #[serde(default = "default_port")]
    chromedriver_port: i32,
}

const DOORDASH_URL: &str = "https://identity.doordash.com/auth/user/signup?client_id=1666519390426295040&enable_last_social=false&intl=en-US&layout=consumer_web&prompt=none&redirect_uri=https%3A%2F%2Fwww.doordash.com%2Fpost-login%2F&response_type=code&scope=%2A&state=%2Fen-US%2Fhome%2F%7C%7Cf0e073b3-2117-4d5e-9129-f5254065cdf3";
const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/96.0.4664.110 Safari/537.36";

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

fn run_chromedriver(path: &str, port: String) -> std::process::Child {
    info!("Starting chromedriver...");
    let output = std::process::Command::new(&path)
        .arg("--ip=localhost")
        .arg(format!("--port={}", port))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .unwrap_or_else(|err| {
            error!("Unable to run chromedriver: {}", err);
            exit(1)
        });
    info!("chromedriver running on port {}", port);
    output
}

async fn get_driver_path() -> Result<String> {
    let browser_name: String = "chrome".to_string();
    let driver_name: String = "chromedriver".to_string();

    let mut selenium_manager: Box<dyn SeleniumManager> = if !browser_name.is_empty() {
        get_manager_by_browser(browser_name).unwrap_or_else(|err| {
            error!("{}", err);
            exit(1);
        })
    } else if !driver_name.is_empty() {
        get_manager_by_driver(driver_name).unwrap_or_else(|err| {
            error!("{}", err);
            exit(1);
        })
    } else {
        error!("You need to specify a browser or driver");
        exit(1);
    };

    let path = match selenium_manager.resolve_driver() {
        Ok(driver_path) => driver_path,
        Err(err) => {
            error!("{}", err);
            exit(1);
        }
    };

    Ok(path.as_os_str().to_str().unwrap().to_string())
}

fn load_config() -> Result<Configuration> {
    let config = Config::builder()
        .add_source(config::File::with_name("config"))
        .add_source(config::Environment::with_prefix("APP"))
        .build()?
        .try_deserialize::<Configuration>()?;

    Ok(config)
}

async fn automate_signup(driver: &WebDriver, config: &Configuration) -> Result<(String, String)> {
    driver.goto(DOORDASH_URL).await?;

    // First name
    driver
        .query(By::Css(
            "input[data-anchor-id=IdentitySignupFirstNameField]",
        ))
        .first()
        .await?
        .send_keys(&config.first_name)
        .await?;

    // Last name
    driver
        .query(By::Css("input[data-anchor-id=IdentitySignupLastNameField]"))
        .first()
        .await?
        .send_keys(&config.last_name)
        .await?;

    // Email
    let email = format!(
        "{}+{}@{}",
        config.email_name,
        thread_rng().gen_range(1000000000..10000000000i64),
        config.email_domain
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
        .send_keys(&config.password)
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
        .send_keys(&config.address)
        .await?;

    driver
        .query(By::Css(
            "span[data-anchor-id=AddressAutocompleteSuggestion-0]",
        ))
        .first()
        .await?
        .click()
        .await?;

    Ok((email, config.password.clone()))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let config = load_config().unwrap_or_else(|err| {
        error!("Failed to load config: {}", err);
        exit(1)
    });
    info!("Config loaded");

    info!("Grabbing chromedriver...");
    let driver_path = tokio::task::spawn_blocking(get_driver_path)
        .await
        .unwrap_or_else(|err| {
            error!("Unable to load driver: {}", err);
            exit(1)
        })
        .await?;

    let mut chromedriver =
        run_chromedriver(driver_path.as_str(), config.chromedriver_port.to_string());

    let mut caps = DesiredCapabilities::chrome();

    caps.add_chrome_arg("--window-size=1920,1080")?;
    caps.add_chrome_arg("--start-maximized")?;
    caps.add_chrome_arg(format!("--user-agent={}", USER_AGENT).as_str())?;

    if config.headless.unwrap_or(true) {
        caps.add_chrome_arg("--headless")?;
    }

    for i in 0..config.quantity.unwrap_or(1) {
        let start = Instant::now();

        let driver = WebDriver::new(
            format!("http://localhost:{}", config.chromedriver_port).as_str(),
            caps.clone(),
        )
        .await?;

        info!(
            "Creating account {} of {}...",
            i + 1,
            config.quantity.unwrap_or(1)
        );
        match automate_signup(&driver, &config).await {
            Ok((email, password)) => info!(
                "Account generated successfully: {}:{}. Took {:?}s",
                email,
                password,
                start.elapsed().as_secs_f32()
            ),
            Err(err) => error!("Failed to generate account: {}", err),
        };

        driver.quit().await?;
    }

    info!("Killing chromedriver...");
    chromedriver
        .kill()
        .expect("unable to kill chromedriver process.");

    Ok(())
}
