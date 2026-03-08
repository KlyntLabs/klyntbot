-- Add unique constraint for upsert-by-type support
CREATE UNIQUE INDEX IF NOT EXISTS idx_coaching_strategies_type_domain
    ON coaching_strategies(strategy_type, domain);
