CREATE TABLE instance_configuration (
    singleton BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (singleton),
    low_stock_threshold SMALLINT NOT NULL
        CHECK (low_stock_threshold BETWEEN 0 AND 100)
);
