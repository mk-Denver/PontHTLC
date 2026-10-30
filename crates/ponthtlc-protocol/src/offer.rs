use bitcoin::hashes::Hash;
use crate::constants::{KIND_OFFER, OFFER_PLATFORM, SWAP_PROFILE};
use crate::event::{is_hex64, NostrEvent};
use crate::swap_v1::{Direction, SwapTerms};

pub struct OfferSide {
    pub price: String,
    pub min: String,
    pub max: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OfferStatus {
    Pending,
    Canceled,
}

impl OfferStatus {
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "canceled" => Some(Self::Canceled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct OfferContent {
    pub direction: Direction,
    pub currency: String,
    pub channels: Vec<String>,
    pub channel_labels: Vec<String>,
    pub network: String,
    pub layer: String,
    pub price: String,
    pub min: String,
    pub max: String,
    pub premium: String,
    pub escrow: String,
    pub resolver: String,
    pub expires_at: u64,
    pub status: OfferStatus,
    pub name: Option<String>,
    pub source: Option<String>,
}

#[derive(Clone, Debug)]
pub struct Offer {
    pub content: OfferContent,
    pub event: NostrEvent,
    pub agent: String,
    pub raw: String,
}

pub fn order_type(direction: &Direction) -> &'static str {
    match direction {
        Direction::FiatToBtc => "sell",
        Direction::BtcToFiat => "buy",
    }
}

pub fn offer_d_tag(currency: &str, direction: &Direction) -> String {
    format!("{SWAP_PROFILE}:{currency}:{}", order_type(direction))
}

pub fn parse_offer(event: &NostrEvent, verify: bool) -> Result<Offer, String> {
    if event.kind != KIND_OFFER {
        return Err("Not an order".to_string());
    }
    if event.tag_value("y") != Some(OFFER_PLATFORM) {
        return Err("Not a Pontmore offer".to_string());
    }
    if event.tag_value("z") != Some("order") {
        return Err("Not a Pontmore order".to_string());
    }
    if event.tag_value("protocol") != Some(SWAP_PROFILE) {
        return Err("Unsupported offer profile".to_string());
    }
    if verify && !event.verify() {
        return Err("Bad offer signature".to_string());
    }

    let k = event.tag_value("k").ok_or("Missing order type")?;
    if k != "sell" && k != "buy" {
        return Err("Bad order type".to_string());
    }
    let direction = if k == "sell" {
        Direction::FiatToBtc
    } else {
        Direction::BtcToFiat
    };
    let currency = event.tag_value("f").ok_or("Missing currency")?;
    if !currency.chars().all(|c| c.is_ascii_uppercase()) || currency.len() != 3 {
        return Err("Bad currency".to_string());
    }
    let expected_d = offer_d_tag(currency, &direction);
    if event.tag_value("d") != Some(&expected_d) {
        return Err("Bad offer slot".to_string());
    }

    let status = OfferStatus::from_str(event.tag_value("s").ok_or("Missing status")?)
        .ok_or("Bad status".to_string())?;

    let fa_tag = event.tag("fa").ok_or("Missing fiat amount range")?;
    let (min, max) = if fa_tag.len() == 3 {
        (fa_tag[1].clone(), fa_tag[2].clone())
    } else {
        (fa_tag[1].clone(), fa_tag[1].clone())
    };
    if !is_positive_decimal(&min) || !is_positive_decimal(&max) {
        return Err("Bad fiat amount".to_string());
    }
    if compare_decimal(&min, &max) > 0 {
        return Err("min above max".to_string());
    }

    let price = event.tag_value("price").ok_or("Missing price")?;
    if !is_positive_decimal(price) {
        return Err("Bad price".to_string());
    }

    if event.tag_value("amt") != Some("0") {
        return Err("Pontmore offers are fiat-denominated".to_string());
    }

    let channels_tag = event.tag("channels").unwrap_or(&[]);
    let channels: Vec<String> = channels_tag.iter().skip(1).cloned().collect();
    if !channels.iter().all(|c| is_valid_channel_id(c)) {
        return Err("Bad channels".to_string());
    }
    if status == OfferStatus::Pending && channels.is_empty() {
        return Err("Offer has no channels".to_string());
    }

    let pm_tag = event.tag("pm").unwrap_or(&[]);
    let channel_labels: Vec<String> = pm_tag.iter().skip(1).cloned().collect();

    let escrow_a = event.tags_by_name("a").into_iter().find(|t| t.get(3) == Some(&"escrow".to_string()));
    let escrow = escrow_a
        .and_then(|t| t.get(1))
        .ok_or("Missing escrow reference")?;
    if !escrow.starts_with("30361:") || !escrow.contains(':') {
        return Err("Bad escrow".to_string());
    }

    let resolver = event
        .tags_by_name("p")
        .into_iter()
        .find(|t| t.get(3) == Some(&"core/resolver".to_string()))
        .and_then(|t| t.get(1))
        .ok_or("Missing resolver")?;
    if !is_hex64(resolver) {
        return Err("Bad resolver".to_string());
    }

    let expires_at = event
        .tag_value("expires_at")
        .and_then(|s| s.parse::<u64>().ok())
        .ok_or("Bad expiry".to_string())?;

    let network = event.tag_value("network").unwrap_or("").to_string();
    let layer = event.tag_value("layer").unwrap_or("").to_string();

    Ok(Offer {
        content: OfferContent {
            direction,
            currency: currency.to_string(),
            channels,
            channel_labels,
            network,
            layer,
            price: price.to_string(),
            min,
            max,
            premium: event.tag_value("premium").unwrap_or("0").to_string(),
            escrow: escrow.to_string(),
            resolver: resolver.to_string(),
            expires_at,
            status,
            name: event.tag_value("name").map(|s| s.to_string()),
            source: event.tag_value("source").map(|s| s.to_string()),
        },
        event: event.clone(),
        agent: event.pubkey.clone(),
        raw: serde_json::to_string(event).map_err(|e| e.to_string())?,
    })
}

pub fn escrow_pk_of(offer: &OfferContent) -> &str {
    offer.escrow.split(':').nth(1).unwrap_or("")
}

pub fn is_offer_live(offer: &OfferContent, at: u64) -> bool {
    offer.status == OfferStatus::Pending && offer.expires_at > at
}

pub struct Quote {
    pub direction: Direction,
    pub fiat_amount: String,
    pub sats: u64,
    pub price: String,
}

pub fn quote(offer: &OfferContent, direction: &Direction, fiat_amount: &str) -> Option<Quote> {
    if offer.direction != *direction {
        return None;
    }
    let sats = sats_for_fiat(fiat_amount, &offer.price)?;
    if sats == 0 {
        return None;
    }
    Some(Quote {
        direction: direction.clone(),
        fiat_amount: fiat_amount.to_string(),
        sats,
        price: offer.price.clone(),
    })
}

pub fn terms_from_offer(
    offer: &OfferContent,
    direction: &Direction,
    fiat_amount: &str,
    channel: &str,
    pay_by: u64,
    confirm_by: u64,
) -> Option<SwapTerms> {
    let q = quote(offer, direction, fiat_amount)?;
    if q.sats == 0 {
        return None;
    }
    Some(SwapTerms {
        direction: direction.clone(),
        fiat: crate::swap_v1::FiatTerms {
            currency: offer.currency.clone(),
            amount: fiat_amount.to_string(),
        },
        bitcoin: crate::swap_v1::BitcoinTerms {
            amount: q.sats.to_string(),
            unit: "sat".to_string(),
            network: offer.layer.clone(),
        },
        payment_channel: channel.to_string(),
        deadlines: crate::swap_v1::DeadlineTerms {
            fiat_pay_by: pay_by,
            fiat_confirm_by: confirm_by,
        },
    })
}

pub fn serialize_offer(event: &NostrEvent) -> String {
    serde_json::to_string(event).unwrap_or_default()
}

fn is_positive_decimal(s: &str) -> bool {
    !s.is_empty()
        && s.chars().all(|c| c.is_ascii_digit() || c == '.')
        && s.parse::<f64>().map(|v| v > 0.0).unwrap_or(false)
}

fn compare_decimal(a: &str, b: &str) -> i32 {
    let av: f64 = a.parse().unwrap_or(0.0);
    let bv: f64 = b.parse().unwrap_or(0.0);
    if av < bv {
        -1
    } else if av > bv {
        1
    } else {
        0
    }
}

fn is_valid_channel_id(s: &str) -> bool {
    !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '@')
        && s.contains('@')
}

fn sats_for_fiat(fiat_amount: &str, price: &str) -> Option<u64> {
    let fiat: f64 = fiat_amount.parse().ok()?;
    let price: f64 = price.parse().ok()?;
    if price <= 0.0 {
        return None;
    }
    let btc = fiat / price;
    let sats = (btc * 100_000_000.0).round() as u64;
    Some(sats)
}

pub fn check_root_against_offer(
    root: &crate::swap_v1::SwapRoot,
    offer: &Offer,
) -> Result<(), String> {
    let terms = &root.terms;
    let offer_content = &offer.content;

    if terms.direction != offer_content.direction {
        return Err("direction mismatch".to_string());
    }
    if terms.fiat.currency != offer_content.currency {
        return Err("currency mismatch".to_string());
    }
    if !offer_content.channels.iter().any(|c| c == &terms.payment_channel) {
        return Err("payment channel not in offer".to_string());
    }
    if terms.bitcoin.network != offer_content.layer {
        return Err("network/layer mismatch".to_string());
    }
    let q = quote(offer_content, &terms.direction, &terms.fiat.amount)
        .ok_or("quote out of range or wrong direction")?;
    if q.sats.to_string() != terms.bitcoin.amount {
        return Err("bitcoin amount does not match quote".to_string());
    }

    if let Some(quote_commitment) = root.commitments.get("quote") {
        let raw = &offer.raw;
        let computed = format!(
            "sha256:{}",
            hex::encode(
                bitcoin::hashes::sha256::Hash::hash(raw.as_bytes()).to_byte_array()
            )
        );
        if quote_commitment.digest != computed {
            return Err("quote commitment mismatch".to_string());
        }
    }

    if root.escrow != *escrow_pk_of(offer_content) {
        return Err("escrow pubkey mismatch".to_string());
    }
    if root.resolver != offer_content.resolver {
        return Err("resolver mismatch".to_string());
    }
    Ok(())
}

pub fn price_still_honoured(
    offer: &OfferContent,
    current_price: &str,
    tolerance_pct: u64,
) -> bool {
    let quoted: f64 = offer.price.parse().unwrap_or(0.0);
    let current: f64 = current_price.parse().unwrap_or(0.0);
    if quoted <= 0.0 || current <= 0.0 {
        return false;
    }
    let tolerance = 1.0 + (tolerance_pct as f64 / 100.0);
    if offer.direction == Direction::FiatToBtc {
        current <= quoted * tolerance
    } else {
        current >= quoted / tolerance
    }
}