// tools/simulator/modules/finance.ts
import type { SimulatorModule, DayContext } from "./types";
import type { World, Ref } from "../world";
import type { ApiClient } from "../client";
import { formatDate } from "../utils/dates";
import { pick, randomCents, randomBetween, shuffle } from "../utils/random";

interface CreateResponse { id: string; [key: string]: unknown }

const WEEKDAY_TRANSACTIONS = [
    { category: "food", subcategory: "lunch", counterparty: "Corner Deli", minDollars: 8, maxDollars: 18 },
    { category: "food", subcategory: "coffee", counterparty: "Blue Bottle Coffee", minDollars: 4, maxDollars: 8 },
    { category: "transport", subcategory: "metro", counterparty: "Metro Transit", minDollars: 2, maxDollars: 6 },
    { category: "food", subcategory: "groceries", counterparty: "Whole Foods", minDollars: 25, maxDollars: 80 },
    { category: "shopping", subcategory: "electronics", counterparty: "Amazon", minDollars: 15, maxDollars: 60 },
    { category: "utilities", subcategory: "phone", counterparty: "Verizon", minDollars: 45, maxDollars: 45 },
    { category: "food", subcategory: "dinner", counterparty: "Thai Palace", minDollars: 15, maxDollars: 35 },
    { category: "health", subcategory: "gym", counterparty: "Planet Fitness", minDollars: 25, maxDollars: 25 },
];

const WEEKEND_TRANSACTIONS = [
    { category: "food", subcategory: "brunch", counterparty: "Cafe de Flore", minDollars: 20, maxDollars: 45 },
    { category: "entertainment", subcategory: "movies", counterparty: "AMC Theaters", minDollars: 12, maxDollars: 25 },
    { category: "shopping", subcategory: "books", counterparty: "Booksmith", minDollars: 10, maxDollars: 35 },
    { category: "food", subcategory: "groceries", counterparty: "Trader Joe's", minDollars: 30, maxDollars: 90 },
];

const PARIS_EXPENSES = [
    { category: "travel", subcategory: "accommodation", counterparty: "Hotel Marais Paris", minDollars: 150, maxDollars: 250 },
    { category: "travel", subcategory: "flights", counterparty: "Air France", minDollars: 400, maxDollars: 800 },
    { category: "travel", subcategory: "activities", counterparty: "Musee d'Orsay", minDollars: 15, maxDollars: 30 },
    { category: "travel", subcategory: "dining", counterparty: "Le Comptoir du Pantheon", minDollars: 25, maxDollars: 60 },
];

export const financeModule: SimulatorModule = {
    name: "finance",
    description: "Accounts, transactions, budgets, investments",
    dependencies: ["para"],

    async seed(world, client) {
        // Create 4 accounts
        const checking = await client.post<CreateResponse>("finance_account_create", {
            name: "Primary Checking",
            accountType: "checking",
            currency: "USD",
            balance: 850000, // $8,500.00 in cents
            institution: "Chase",
        });
        world.accounts.checking = { id: checking.id, title: "Primary Checking" };

        const savings = await client.post<CreateResponse>("finance_account_create", {
            name: "High-Yield Savings",
            accountType: "savings",
            currency: "USD",
            balance: 2500000, // $25,000.00
            institution: "Marcus by Goldman Sachs",
        });
        world.accounts.savings = { id: savings.id, title: "High-Yield Savings" };

        const creditCard = await client.post<CreateResponse>("finance_account_create", {
            name: "Visa Signature",
            accountType: "credit_card",
            currency: "USD",
            balance: -45000, // -$450.00 (balance owed)
            institution: "Chase",
        });
        world.accounts.creditCard = { id: creditCard.id, title: "Visa Signature" };

        const brokerage = await client.post<CreateResponse>("finance_account_create", {
            name: "Brokerage Account",
            accountType: "investment",
            currency: "USD",
            balance: 15000000, // $150,000.00
            institution: "Fidelity",
        });
        world.accounts.brokerage = { id: brokerage.id, title: "Brokerage Account" };
        console.log(`  4 accounts created`);

        // Create 3 budgets
        await client.post<CreateResponse>("finance_budget_create", {
            name: "Groceries & Dining",
            amount: 60000, // $600/month
            period: "monthly",
            category: "food",
        });
        await client.post<CreateResponse>("finance_budget_create", {
            name: "Transport",
            amount: 15000, // $150/month
            period: "monthly",
            category: "transport",
        });
        await client.post<CreateResponse>("finance_budget_create", {
            name: "Entertainment",
            amount: 20000, // $200/month
            period: "monthly",
            category: "entertainment",
        });
        console.log(`  3 budgets created`);

        // Create 2 portfolios
        const retirement = await client.post<CreateResponse>("finance_portfolio_create", {
            name: "Retirement 401k",
            description: "Tax-advantaged retirement",
            currency: "USD",
        });
        const brokeragePortfolio = await client.post<CreateResponse>("finance_portfolio_create", {
            name: "Brokerage",
            description: "General investment account",
            currency: "USD",
        });
        console.log(`  2 portfolios created`);

        // Create investments in each portfolio
        await client.post<CreateResponse>("finance_investment_create", {
            portfolioId: retirement.id,
            assetType: "etf",
            symbol: "VTI",
            name: "Vanguard Total Stock Market",
            costBasis: 1500000,
            quantity: "65.5",
            currency: "USD",
        });
        await client.post<CreateResponse>("finance_investment_create", {
            portfolioId: retirement.id,
            assetType: "etf",
            symbol: "VXUS",
            name: "Vanguard Total International Stock",
            costBasis: 800000,
            quantity: "42.3",
            currency: "USD",
        });
        await client.post<CreateResponse>("finance_investment_create", {
            portfolioId: retirement.id,
            assetType: "bond",
            symbol: "BND",
            name: "Vanguard Total Bond Market",
            costBasis: 500000,
            quantity: "30.0",
            currency: "USD",
        });
        await client.post<CreateResponse>("finance_investment_create", {
            portfolioId: brokeragePortfolio.id,
            assetType: "etf",
            symbol: "VOO",
            name: "Vanguard S&P 500",
            costBasis: 1200000,
            quantity: "28.7",
            currency: "USD",
        });
        await client.post<CreateResponse>("finance_investment_create", {
            portfolioId: brokeragePortfolio.id,
            assetType: "etf",
            symbol: "QQQ",
            name: "Invesco QQQ Trust",
            costBasis: 600000,
            quantity: "15.2",
            currency: "USD",
        });
        await client.post<CreateResponse>("finance_investment_create", {
            portfolioId: brokeragePortfolio.id,
            assetType: "etf",
            symbol: "SCHD",
            name: "Schwab US Dividend Equity",
            costBasis: 400000,
            quantity: "22.1",
            currency: "USD",
        });
        console.log(`  6 investments created across 2 portfolios`);

        // Create finance goals
        await client.post<CreateResponse>("finance_goal_create", {
            name: "Emergency Fund",
            goalType: "savings",
            targetAmount: 2500000,
            currentAmount: 1800000,
            monthlyContribution: 50000,
            currency: "USD",
        });
        await client.post<CreateResponse>("finance_goal_create", {
            name: "Paris Trip Fund",
            goalType: "savings",
            targetAmount: 450000,
            currentAmount: 200000,
            deadline: "2026-03-15",
            currency: "USD",
            monthlyContribution: 25000,
        });
        console.log(`  2 finance goals created`);

        // Create liabilities
        await client.post<CreateResponse>("finance_liability_create", {
            name: "Student Loan",
            liabilityType: "loan",
            principal: 2500000,
            remaining: 1800000,
            interestRate: 4.5,
            monthlyPayment: 28000,
            currency: "USD",
        });
        await client.post<CreateResponse>("finance_liability_create", {
            name: "Credit Card Balance",
            liabilityType: "revolving",
            principal: 45000,
            remaining: 45000,
            interestRate: 19.99,
            monthlyPayment: 15000,
            currency: "USD",
        });
        console.log(`  2 liabilities created`);
    },

    async simulateDay(world, client, day) {
        const date = formatDate(day.date);
        let txCount = 0;
        let totalSpent = 0;

        if (day.isWeekend) {
            // 1-2 weekend transactions
            const count = randomBetween(1, 2);
            for (let i = 0; i < count; i++) {
                const tx = pick(WEEKEND_TRANSACTIONS);
                const amount = randomCents(tx.minDollars, tx.maxDollars);
                await client.post("finance_transaction_create", {
                    accountId: world.accounts.creditCard.id,
                    txType: "debit",
                    amount,
                    category: tx.category,
                    subcategory: tx.subcategory,
                    counterparty: tx.counterparty,
                    txDate: date,
                    notes: `Weekend ${tx.subcategory}`,
                });
                txCount++;
                totalSpent += amount;
            }
        } else {
            // 2-5 weekday transactions
            const count = randomBetween(2, 5);
            const txPool = shuffle([...WEEKDAY_TRANSACTIONS]);
            for (let i = 0; i < Math.min(count, txPool.length); i++) {
                const tx = txPool[i];
                const amount = randomCents(tx.minDollars, tx.maxDollars);
                const accountId = i < 2 ? world.accounts.checking.id : world.accounts.creditCard.id;
                await client.post("finance_transaction_create", {
                    accountId,
                    txType: "debit",
                    amount,
                    category: tx.category,
                    subcategory: tx.subcategory,
                    counterparty: tx.counterparty,
                    txDate: date,
                });
                txCount++;
                totalSpent += amount;
            }

            // Wednesday: investment contribution
            if (day.dayOfWeek === 2) {
                const investAmount = randomCents(300, 500);
                await client.post("finance_transaction_create", {
                    accountId: world.accounts.brokerage.id,
                    txType: "credit",
                    amount: investAmount,
                    category: "investing",
                    subcategory: "contribution",
                    counterparty: "Fidelity Auto-Invest",
                    txDate: date,
                    notes: "Weekly auto-investment",
                });
                txCount++;
                console.log(`  finance: investment contribution $${(investAmount / 100).toFixed(2)}`);
            }

            // Friday: Paris trip expense
            if (day.dayOfWeek === 4) {
                const parisTx = pick(PARIS_EXPENSES);
                const amount = randomCents(parisTx.minDollars, parisTx.maxDollars);
                await client.post("finance_transaction_create", {
                    accountId: world.accounts.creditCard.id,
                    txType: "debit",
                    amount,
                    category: parisTx.category,
                    subcategory: parisTx.subcategory,
                    counterparty: parisTx.counterparty,
                    txDate: date,
                    notes: `Paris trip: ${parisTx.subcategory}`,
                });
                txCount++;
                totalSpent += amount;
                console.log(`  finance: Paris trip expense $${(amount / 100).toFixed(2)} at ${parisTx.counterparty}`);
            }
        }

        console.log(`  finance: ${txCount} transactions, $${(totalSpent / 100).toFixed(2)} spent`);
    },
};
