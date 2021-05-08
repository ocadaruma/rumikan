#[cfg(test)]
mod tests {
    use bit_field::BitField;

    #[test]
    fn test_bit_field() {
        let x = 0x00112000;
        assert_eq!(x.get_bits(16..32) as u16, 17);
    }
}
