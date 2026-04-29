#[derive(Debug, Default, Clone, Copy)]
pub struct CostDashboard {
    pub hot_path_usd_per_turn: f64,
    pub warm_path_usd_per_session: f64,
    pub reforge_usd_per_night: f64,
}

#[derive(Debug, Clone, Copy)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
}

pub const HAIKU_45: ModelPricing = ModelPricing {
    input_per_million: 0.80,
    output_per_million: 4.00,
};
pub const SONNET_46: ModelPricing = ModelPricing {
    input_per_million: 3.0,
    output_per_million: 15.0,
};
pub const KIMI_K2: ModelPricing = ModelPricing {
    input_per_million: 0.15,
    output_per_million: 2.50,
};
pub const DEEPSEEK_V32: ModelPricing = ModelPricing {
    input_per_million: 0.28,
    output_per_million: 0.42,
};

pub fn cost_for(input_tokens: u64, output_tokens: u64, p: ModelPricing) -> f64 {
    (input_tokens as f64 / 1_000_000.0) * p.input_per_million
        + (output_tokens as f64 / 1_000_000.0) * p.output_per_million
}
