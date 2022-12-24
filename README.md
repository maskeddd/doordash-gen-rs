# doordash-account-creator

Automatically creates DoorDash accounts with given details.

## Running

1. Download the latest release from [here](https://github.com/maskeddd/doordash-gen-rs/releases)
2. Create a `config.toml` file in the same directory
3. Run the executable

## Configure

The tool can be configured in the included `config.toml` file. Most items are required.

```toml
first_name = "John"
last_name = "Doe"
email_name = "example"
email_domain = "gmail.com"
address = "303 2nd St, Suite 800 San Francisco"
quantity = 5
headless = true
```
