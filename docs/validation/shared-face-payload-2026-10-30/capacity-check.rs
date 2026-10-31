fn main() {
    for faces in [362494, 602112] {
        let mut uv: Vec<[[f32; 4]; 3]> = Vec::new();
        let mut colors: Vec<[u8; 4]> = Vec::new();
        for _ in 0..faces { uv.push([[0.; 4]; 3]); colors.push([0; 4]); }
        let spare = (uv.capacity() - uv.len()) * std::mem::size_of::<[[f32; 4]; 3]>()
            + (colors.capacity() - colors.len()) * std::mem::size_of::<[u8; 4]>();
        println!("faces={faces}, UV capacity={}, color capacity={}, spare bytes={spare}", uv.capacity(), colors.capacity());
        uv.shrink_to_fit(); colors.shrink_to_fit();
        println!("trimmed UV capacity={}, color capacity={}", uv.capacity(), colors.capacity());
    }
}
