use std::time::Duration;

pub fn lnurlp_url(address: &str) -> Result<String, String> {
    let (user, host) = split_address(address)?;
    Ok(format!("https://{host}/.well-known/lnurlp/{user}"))
}

pub fn callback_with_amount(callback: &str, amount_msat: u64) -> Result<String, String> {
    if !(callback.starts_with("https://") || callback.starts_with("http://")) {
        return Err("lnurl callback must be an http(s) url".to_string());
    }
    if callback.contains("amount=") {
        return Err("lnurl callback already includes an amount".to_string());
    }
    let joiner = if callback.contains('?') { '&' } else { '?' };
    Ok(format!("{callback}{joiner}amount={amount_msat}"))
}

pub fn callback_from_body(body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    reject_lnurl_error(&value)?;
    value
        .get("callback")
        .and_then(|item| item.as_str())
        .filter(|callback| !callback.is_empty())
        .map(|callback| callback.to_string())
        .ok_or_else(|| "lnurl pay response has no callback".to_string())
}

pub fn invoice_from_body(body: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(body).map_err(|err| err.to_string())?;
    reject_lnurl_error(&value)?;
    value
        .get("pr")
        .and_then(|item| item.as_str())
        .filter(|invoice| invoice.starts_with("ln"))
        .map(|invoice| invoice.to_string())
        .ok_or_else(|| "lnurl callback has no invoice".to_string())
}

pub async fn fetch_invoice(address: &str, amount_msat: u64) -> Result<String, String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|err| err.to_string())?;
    let metadata = client
        .get(lnurlp_url(address)?)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .text()
        .await
        .map_err(|err| err.to_string())?;
    let callback = callback_with_amount(&callback_from_body(&metadata)?, amount_msat)?;
    let invoice_body = client
        .get(callback)
        .send()
        .await
        .map_err(|err| err.to_string())?
        .text()
        .await
        .map_err(|err| err.to_string())?;
    invoice_from_body(&invoice_body)
}

fn split_address(address: &str) -> Result<(&str, &str), String> {
    let address = address.trim();
    let (user, host) = address
        .split_once('@')
        .ok_or_else(|| "lightning address must look like name@domain".to_string())?;
    if user.is_empty()
        || host.is_empty()
        || user.contains('@')
        || host.contains('/')
        || host.contains(' ')
        || !host.contains('.')
    {
        return Err("lightning address must look like name@domain".to_string());
    }
    Ok((user, host))
}

fn reject_lnurl_error(value: &serde_json::Value) -> Result<(), String> {
    if value.get("status").and_then(|item| item.as_str()) == Some("ERROR") {
        let reason = value
            .get("reason")
            .and_then(|item| item.as_str())
            .unwrap_or("lnurl pay failed");
        return Err(reason.to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_lnurlp_and_callback_urls() {
        assert_eq!(
            lnurlp_url("buyer@walletofsatoshi.com").unwrap(),
            "https://walletofsatoshi.com/.well-known/lnurlp/buyer"
        );
        assert!(lnurlp_url("not-an-address").is_err());
        assert_eq!(
            callback_with_amount("https://walletofsatoshi.com/lnurlp/buyer", 50_000).unwrap(),
            "https://walletofsatoshi.com/lnurlp/buyer?amount=50000"
        );
        assert!(callback_with_amount("file:///tmp/invoice", 1).is_err());
    }

    #[test]
    fn reads_callback_and_invoice() {
        let callback = callback_from_body(
            r#"{"callback":"https://example.com/pay","minSendable":1000,"maxSendable":1000000}"#,
        )
        .unwrap();
        assert_eq!(callback, "https://example.com/pay");
        let invoice = invoice_from_body(r#"{"pr":"lnbc10n1example"}"#).unwrap();
        assert_eq!(invoice, "lnbc10n1example");
        assert!(invoice_from_body(r#"{"status":"ERROR","reason":"amount out of range"}"#).is_err());
    }
}
