#![macro_escape]

#[macro_export]
macro_rules! likely(
    ($val:expr) => {
        {
            extern {
                #[link_name = "llvm.expect.i8"]
                fn expect(val: u8, expected_val: u8) -> u8;
            }
            let x: bool = $val;
            unsafe { expect(x as u8, 1) != 0 }
        }
    }
)

#[macro_export]
macro_rules! unlikely(
    ($val:expr) => {
        {
            extern {
                #[link_name = "llvm.expect.i8"]
                fn expect(val: u8, expected_val: u8) -> u8;
            }
            let x: bool = $val;
            unsafe { expect(x as u8, 0) != 0 }
        }
    }
)
