INSERT INTO areas (id, name, color, position) VALUES
  ('work', 'Work', '#60a5fa', 0),
  ('personal', 'Personal', '#34d399', 1);

INSERT INTO projects (id, area_id, name, color) VALUES
  ('p1', 'work', 'Klyntbot Desktop', '#f97316'),
  ('p2', 'work', 'API Redesign', '#60a5fa'),
  ('p3', 'work', 'Marketing Site', '#a78bfa'),
  ('p4', 'personal', 'Home Renovation', '#34d399'),
  ('p5', 'personal', 'Fitness Plan', '#f43f5e');

INSERT INTO actions (id, title, area_id, project_id, priority, status, due_date, tags, completed_at) VALUES
  ('t1',  'Implement glassmorphism theme',      'work', 'p1', 1, 'doing', '2026-03-10', '["ui","design"]', NULL),
  ('t2',  'Set up CI/CD pipeline',              'work', 'p1', 2, 'done',  '2026-03-01', '["devops"]', '2026-03-01T10:00:00Z'),
  ('t3',  'Write unit tests for agent runtime', 'work', 'p1', 2, 'doing', '2026-03-12', '["testing"]', NULL),
  ('t4',  'Design new onboarding flow',         'work', 'p3', 1, 'todo',  '2026-03-15', '["ux"]', NULL),
  ('t5',  'Migrate database to new schema',     'work', 'p2', 1, 'todo',  '2026-03-08', '["backend"]', NULL),
  ('t6',  'Optimize query performance',         'work', 'p2', 2, 'doing', NULL,          '["performance"]', NULL),
  ('t7',  'Add keyboard shortcuts',             'work', 'p1', 3, 'todo',  '2026-03-20', '["ux"]', NULL),
  ('t8',  'Review PR #247',                     'work', 'p1', 2, 'done',  '2026-03-04', '["review"]', '2026-03-04T14:00:00Z'),
  ('t9',  'Fix memory leak in streaming',       'work', 'p1', 1, 'doing', '2026-03-06', '["bug"]', NULL),
  ('t10', 'Update API documentation',           'work', 'p2', 3, 'todo',  NULL,          '["docs"]', NULL),
  ('t11', 'Paint living room',                  'personal', 'p4', 2, 'done', '2026-02-28', '["home"]', '2026-02-28T16:00:00Z'),
  ('t12', 'Install new shelves',                'personal', 'p4', 2, 'todo', '2026-03-18', '["home"]', NULL),
  ('t13', 'Run 5K three times this week',       'personal', 'p5', 2, 'doing', '2026-03-07', '["health"]', NULL),
  ('t14', 'Meal prep for the week',             'personal', 'p5', 3, 'done', '2026-03-03', '["health"]', '2026-03-03T08:00:00Z'),
  ('t15', 'Write blog post about Tahoe design', 'work', 'p3', 3, 'todo', '2026-03-25', '["writing"]', NULL);

INSERT INTO actions (id, title, area_id, project_id, parent_id, priority, status, tags, completed_at) VALUES
  ('t1s1', 'Define glass CSS variables',   'work', 'p1', 't1', 2, 'done', '["ui"]', '2026-03-04T12:00:00Z'),
  ('t1s2', 'Update all card components',   'work', 'p1', 't1', 2, 'doing', '["ui"]', NULL),
  ('t1s3', 'Add background gradient mesh', 'work', 'p1', 't1', 2, 'todo', '["ui"]', NULL);

INSERT INTO finance_accounts (id, name, account_type, currency, balance, institution, notes) VALUES
  ('fa1', 'Checking Account', 'checking', 'VND', 45200000,   'Vietcombank', NULL),
  ('fa2', 'Savings',          'savings',  'VND', 182500000,  'Techcombank', NULL),
  ('fa3', 'USD Account',      'checking', 'USD', 325000,     'Wise', NULL),
  ('fa4', 'Emergency Fund',   'savings',  'VND', 95000000,   'MB Bank', '6 months expenses'),
  ('fa5', 'Credit Card',      'credit',   'VND', -8750000,   'Vietcombank', NULL);

INSERT INTO finance_transactions (id, account_id, tx_type, amount, currency, category, subcategory, counterparty, notes, tx_date) VALUES
  ('ft1',  'fa1', 'expense',  2500000,  'VND', 'Food & Dining',   'Restaurants',  'Pizza 4Ps',        NULL,             '2026-03-05'),
  ('ft2',  'fa1', 'expense',  850000,   'VND', 'Transportation',  'Grab',         'Grab',             NULL,             '2026-03-05'),
  ('ft3',  'fa1', 'income',   42000000, 'VND', 'Salary',          NULL,           'Klynt Inc',        'March salary',   '2026-03-01'),
  ('ft4',  'fa1', 'expense',  12000000, 'VND', 'Housing',         'Rent',         'Landlord',         NULL,             '2026-03-01'),
  ('ft5',  'fa2', 'transfer', 10000000, 'VND', NULL,              NULL,           NULL,               'Monthly savings', '2026-03-02'),
  ('ft6',  'fa1', 'expense',  1200000,  'VND', 'Shopping',        'Electronics',  'Shopee',           'USB-C hub',      '2026-03-04'),
  ('ft7',  'fa1', 'expense',  450000,   'VND', 'Food & Dining',   'Coffee',       'The Coffee House', NULL,             '2026-03-04'),
  ('ft8',  'fa3', 'income',   50000,    'USD', 'Freelance',       NULL,           'Client ABC',       'Consulting',     '2026-03-03'),
  ('ft9',  'fa1', 'expense',  3500000,  'VND', 'Utilities',       'Internet',     'VNPT',             NULL,             '2026-03-01'),
  ('ft10', 'fa5', 'expense',  5200000,  'VND', 'Shopping',        'Clothing',     'Uniqlo',           NULL,             '2026-03-03');

INSERT INTO finance_budgets (id, name, amount, currency, period, category, method, jar_type, alert_threshold) VALUES
  ('b1', 'Food & Dining',  8000000,  'VND', 'monthly', 'Food & Dining',  'envelope',            'necessities',      80),
  ('b2', 'Transportation', 3000000,  'VND', 'monthly', 'Transportation', 'envelope',            'necessities',      80),
  ('b3', 'Shopping',       5000000,  'VND', 'monthly', 'Shopping',       'envelope',            'play',             90),
  ('b4', 'Entertainment',  3000000,  'VND', 'monthly', 'Entertainment',  'envelope',            'play',             80),
  ('b5', 'Savings',        15000000, 'VND', 'monthly', NULL,             'pay_yourself_first',  'long_term_savings', 50);

INSERT INTO finance_portfolios (id, name, description, currency) VALUES
  ('fp1', 'Growth Portfolio', 'Long-term growth stocks', 'VND'),
  ('fp2', 'Crypto',          'Digital assets',           'USD');

INSERT INTO finance_investments (id, portfolio_id, asset_type, symbol, name, quantity, cost_basis, currency, current_price, current_value) VALUES
  ('fi1', 'fp1', 'stock',  'VNM',      'Vinamilk',        500,  42500000,  'VND', 95000,    47500000),
  ('fi2', 'fp1', 'stock',  'FPT',      'FPT Corporation', 300,  36000000,  'VND', 145000,   43500000),
  ('fi3', 'fp1', 'etf',    'FUEVFVND', 'VN30 ETF',        2000, 32000000,  'VND', 18500,    37000000),
  ('fi4', 'fp1', 'stock',  'MWG',      'Mobile World',    800,  48000000,  'VND', 68000,    54400000),
  ('fi5', 'fp1', 'bond',   NULL,       'Gov Bond 5Y',     1,    100000000, 'VND', NULL,     102600000),
  ('fi6', 'fp2', 'crypto', 'BTC',      'Bitcoin',         0.05, 320000,    'USD', 10500000, 525000),
  ('fi7', 'fp2', 'crypto', 'ETH',      'Ethereum',        0.8,  200000,    'USD', 340000,   272000),
  ('fi8', 'fp2', 'crypto', 'SOL',      'Solana',          5,    100000,    'USD', 10600,    53000);

INSERT INTO finance_goals (id, name, goal_type, target_amount, current_amount, currency, deadline, monthly_contribution) VALUES
  ('fg1', 'Emergency Fund', 'emergency_fund', 120000000, 95000000, 'VND', '2026-06-30', 5000000),
  ('fg2', 'MacBook Pro',    'purchase',       65000000,  28000000, 'VND', '2026-09-01', 8000000),
  ('fg3', 'Japan Trip',     'travel',         40000000,  15500000, 'VND', '2026-12-15', 3000000);

INSERT INTO finance_liabilities (id, name, liability_type, principal, remaining, currency, interest_rate, monthly_payment, due_date) VALUES
  ('fl1', 'Credit Card',  'credit_card',   8750000,  8750000,  'VND', 18.0, 8750000, '2026-03-25'),
  ('fl2', 'Student Loan', 'personal_loan', 50000000, 32000000, 'VND', 8.5,  2500000, '2028-06-01');
