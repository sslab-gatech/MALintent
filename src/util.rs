/// Encodes bytes into a hexstring like \x41\x42\x43
pub fn encode_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("\\x{:02x}", b)).collect()
}

/// Array that contains common extra keys and types.
pub const COMMON_EXTRA_KEYS: [(&str, &str); 14] = [
    ("android.intent.extra.CC", "StringArray"),
    ("android.intent.extra.COMPONENT_NAME", "ComponentName"),
    ("android.intent.extra.EMAIL", "StringArray"),
    ("android.intent.extra.HTML_TEXT", "String"),
    ("android.intent.extra.INDEX", "Int"),
    // ("android.intent.extra.INITIAL_INTENTS", "ParcelableArray"),
    ("android.intent.extra.MIME_TYPES", "StringArray"),
    ("android.intent.extra.PACKAGE_NAME", "String"),
    ("android.intent.extra.PHONE_NUMBER", "String"),
    ("android.intent.extra.QUICK_VIEW_FEATURES", "StringArray"),
    ("android.intent.extra.STREAM", "URI"),
    ("android.intent.extra.SUBJECT", "String"),
    ("android.intent.extra.TEXT", "String"),
    ("android.intent.extra.TITLE", "String"),
    ("android.intent.extra.UID", "Int"),
];
