use fireworks_collaboration_lib::core::tls::verifier::validate_pins;

#[test]
fn test_validate_pins_valid() {
    // Valid SHA256 base64url encoded pins (length 43)
    // SHA256("") = e3b0c442... -> Base64URL: 47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU
    let valid_pin = "47DEQpj8HBSa-_TImW-5JCeuQeRkm5NMpJWZG3hSuFU".to_string();
    let pins = vec![valid_pin.clone()];

    let result = validate_pins(&pins);
    assert!(result.is_some(), "Should validate correct pin");
    let validated = result.unwrap();
    assert_eq!(validated.len(), 1);
    assert_eq!(validated[0], valid_pin);
}

#[test]
fn test_validate_pins_empty() {
    let pins: Vec<String> = vec![];
    let result = validate_pins(&pins);
    assert!(result.is_some());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_validate_pins_invalid_length() {
    let pins = vec![
        "TooShort".to_string(), // < 43
    ];
    assert!(validate_pins(&pins).is_none());

    let pins2 = vec![
        "TooLongAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string(), // > 43
    ];
    assert!(validate_pins(&pins2).is_none());
}

#[test]
fn test_validate_pins_invalid_encoding() {
    // Contains invalid characters for base64url (e.g., '+')
    let pins = vec!["Invalid+EncodingAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string()];
    assert!(validate_pins(&pins).is_none());
}

#[test]
fn test_validate_pins_too_many() {
    let mut pins = Vec::new();
    for _ in 0..11 {
        pins.push("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string());
    }
    assert!(validate_pins(&pins).is_none());
}

#[test]
fn test_validate_pins_deduplication() {
    let pin = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA".to_string();
    let pins = vec![pin.clone(), pin.clone()];

    let result = validate_pins(&pins);
    assert!(result.is_some());
    let validated = result.unwrap();
    assert_eq!(validated.len(), 1);
    assert_eq!(validated[0], pin);
}
