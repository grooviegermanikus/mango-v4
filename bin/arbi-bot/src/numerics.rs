use fixed::types::I80F48;
use mango_v4::state::{PerpMarket, QUOTE_DECIMALS};

impl ConversionConf for PerpMarket {
    fn get_base_decimals(&self) -> u32 {
        self.base_decimals.into()
    }

    fn get_base_lot_size(&self) -> i64 {
        self.base_lot_size
    }

    fn get_quote_lot_size(&self) -> i64 {
        self.quote_lot_size
    }
}

pub trait ConversionConf {
    fn get_base_decimals(&self) -> u32;
    fn get_base_lot_size(&self) -> i64;
    fn get_quote_lot_size(&self) -> i64;

}

pub fn native_amount_to_lot(lot_conf: &dyn ConversionConf, amount: f64) -> i64 {
    // base_decimals=6
    // 0.0001 in 1e6(decimals) = 100 = 1 lot
    let order_size = I80F48::from_num(amount);

    let exact = order_size * I80F48::from_num(10u64.pow(lot_conf.get_base_decimals()))
        / I80F48::from_num(lot_conf.get_base_lot_size());

    exact.to_num::<f64>().round() as i64
}

pub fn native_amount(lot_conf: &dyn ConversionConf, amount: f64) -> u64 {
    let order_size = I80F48::from_num(amount);

    let exact = order_size * I80F48::from_num(10u64.pow(lot_conf.get_base_decimals()));

    exact.to_num::<f64>().round() as u64
}


pub fn quote_amount_to_lot(lot_conf: &dyn ConversionConf, amount: f64) -> i64 {
    // quote_decimals always 6
    let order_size = I80F48::from_num(amount);

    let exact = order_size * I80F48::from_num(10u64.pow(QUOTE_DECIMALS as u32))
        / I80F48::from_num(lot_conf.get_quote_lot_size());

    exact.to_num::<f64>().round() as i64
}


// base
pub fn quantity_to_lot(lot_conf: &dyn ConversionConf, amount: f64) -> I80F48 {
    // base_decimals=6
    // 0.0001 in 1e6(decimals) = 100 = 1 lot
    let order_size = I80F48::from_num(amount);

    order_size * I80F48::from_num(10u64.pow(lot_conf.get_base_decimals()))
        / I80F48::from_num(lot_conf.get_base_lot_size())
}

mod test {
    use crate::numerics::{ConversionConf, native_amount, native_amount_to_lot, quantity_to_lot, quote_amount_to_lot};

    #[test]
    fn convert_quantity_eth_perp() {

        struct Sample;

        impl ConversionConf for Sample {
            fn get_base_decimals(&self) -> u32 {
                6
            }

            fn get_base_lot_size(&self) -> i64 {
                100
            }

            fn get_quote_lot_size(&self) -> i64 {
                10
            }
        }

        assert_eq!(1, native_amount_to_lot(&Sample, 0.0001));
        assert_eq!(100, native_amount(&Sample, 0.0001));
        assert_eq!(10, quote_amount_to_lot(&Sample, 0.0001));
        assert_eq!(500 * 1_000_000 / 100, quantity_to_lot(&Sample, 500.00));

        // quantity_to_lot()

    }
}

