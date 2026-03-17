"use client";

import { useState } from "react";

const sections = [
  { id: "what", label: "What is AlgoTrader?" },
  { id: "how", label: "How It Works" },
  { id: "strategies", label: "Your Strategies" },
  { id: "pages", label: "Dashboard Pages" },
  { id: "safety", label: "Safety & Risk Rules" },
  { id: "glossary", label: "Glossary" },
];

export default function GuidePage() {
  const [active, setActive] = useState("what");

  return (
    <div>
      <h2 className="text-2xl font-bold mb-1">Guide</h2>
      <p className="text-gray-500 text-sm mb-6">
        Everything you need to understand how AlgoTrader works, explained in plain English.
      </p>

      {/* Section tabs */}
      <div className="flex flex-wrap gap-2 mb-6">
        {sections.map((s) => (
          <button
            key={s.id}
            onClick={() => setActive(s.id)}
            className={`text-sm px-3 py-1.5 rounded-full transition-colors ${
              active === s.id
                ? "bg-blue-600 text-white"
                : "bg-gray-100 text-gray-600 hover:bg-gray-200"
            }`}
          >
            {s.label}
          </button>
        ))}
      </div>

      <div className="bg-white rounded-lg border border-gray-200 p-6 shadow-sm prose prose-sm max-w-none">
        {active === "what" && <WhatSection />}
        {active === "how" && <HowSection />}
        {active === "strategies" && <StrategiesSection />}
        {active === "pages" && <PagesSection />}
        {active === "safety" && <SafetySection />}
        {active === "glossary" && <GlossarySection />}
      </div>
    </div>
  );
}

function SectionTitle({ children }: { children: React.ReactNode }) {
  return <h3 className="text-lg font-bold text-gray-900 mb-3">{children}</h3>;
}

function P({ children }: { children: React.ReactNode }) {
  return <p className="text-gray-700 leading-relaxed mb-3">{children}</p>;
}

function WhatSection() {
  return (
    <>
      <SectionTitle>What is AlgoTrader?</SectionTitle>
      <P>
        AlgoTrader is your personal automated trading assistant. Instead of you watching stock prices
        all day and deciding when to buy or sell, AlgoTrader does it for you using a set of rules
        (called <strong>strategies</strong>) that run automatically.
      </P>
      <P>
        Think of it like cruise control for stock trading. You set the rules, the system watches
        the market, and when conditions match your rules, it automatically places trades through
        your brokerage account (Alpaca Markets).
      </P>
      <P>
        Right now, AlgoTrader watches 6 popular stocks and ETFs: <strong>SPY</strong> (S&amp;P 500 index),{" "}
        <strong>QQQ</strong> (Nasdaq 100 index), <strong>AAPL</strong> (Apple),{" "}
        <strong>MSFT</strong> (Microsoft), <strong>NVDA</strong> (Nvidia), and{" "}
        <strong>GOOGL</strong> (Google).
      </P>
      <P>
        It only trades during regular market hours (9:30 AM to 4:00 PM Eastern Time, Monday through
        Friday) and automatically sells everything by 3:45 PM to avoid holding positions overnight.
      </P>
      <div className="bg-yellow-50 border border-yellow-200 rounded-lg p-4 text-sm text-yellow-800 mt-4">
        <strong>Important:</strong> AlgoTrader starts in &quot;paper trading&quot; mode, which means it uses
        fake money to simulate real trades. This lets you see how the strategies perform without
        risking real money. Never switch to live mode until you are confident in the results.
      </div>
    </>
  );
}

function HowSection() {
  return (
    <>
      <SectionTitle>How It Works</SectionTitle>
      <P>
        AlgoTrader has three parts that work together, like a factory assembly line:
      </P>

      <div className="space-y-4 mb-4">
        <div className="bg-blue-50 border border-blue-200 rounded-lg p-4">
          <h4 className="font-semibold text-blue-900 mb-1">1. Market Data Arrives</h4>
          <p className="text-sm text-blue-800">
            Every 5 minutes during market hours, your brokerage (Alpaca) sends the latest price data
            for all 6 stocks. This includes the open, high, low, close prices, and trading volume
            for that 5-minute window. This is stored in a local database on your computer.
          </p>
        </div>

        <div className="bg-green-50 border border-green-200 rounded-lg p-4">
          <h4 className="font-semibold text-green-900 mb-1">2. Strategies Analyze the Data</h4>
          <p className="text-sm text-green-800">
            Each new price update is sent to the Strategy Engine, where all your active strategies
            look at the data and decide: should I BUY, SELL, or do nothing (HOLD)? Each strategy
            also reports how confident it is in its decision (0% to 100%).
          </p>
        </div>

        <div className="bg-purple-50 border border-purple-200 rounded-lg p-4">
          <h4 className="font-semibold text-purple-900 mb-1">3. Risk Checks and Execution</h4>
          <p className="text-sm text-purple-800">
            Before any trade is placed, the signal passes through safety checks (risk rules).
            If the trade is too risky — for example, if you have already lost too much today, or
            the strategy is not confident enough — the trade is blocked. If it passes all checks,
            the order is sent to Alpaca and executed.
          </p>
        </div>
      </div>

      <P>
        The whole cycle — data in, analysis, risk check, trade — happens automatically every
        5 minutes for each stock. You can watch it all happening in real time on this dashboard.
      </P>
    </>
  );
}

function StrategiesSection() {
  return (
    <>
      <SectionTitle>Your Strategies Explained</SectionTitle>
      <P>
        You have four strategies, each using a different approach to decide when to trade.
        Having multiple strategies is like getting opinions from different experts — sometimes
        they agree, sometimes they do not.
      </P>

      <div className="space-y-4">
        <StrategyExplainer
          name="Moving Average Crossover"
          emoji="📊"
          simple={`Follows the trend. If a stock's recent average price climbs above its longer-term average, the trend is going up — time to buy. When the short-term average drops below the long-term average, the trend is reversing — time to sell.`}
          example={`If Apple's 10-day average price crosses above its 30-day average, the strategy says "BUY" because prices are trending upward.`}
        />

        <StrategyExplainer
          name="RSI Mean Reversion"
          emoji="🔄"
          simple={`Looks for stocks that have been pushed too far in one direction and are likely to "bounce back." If a stock has been falling hard (oversold), it bets the price will recover. If a stock has been rising too fast (overbought), it expects a pullback.`}
          example={`If Nvidia's RSI drops below 30 (heavily oversold), the strategy says "BUY" expecting a bounce. If RSI goes above 70 (overbought), it says "SELL."`}
        />

        <StrategyExplainer
          name="Momentum Volume"
          emoji="🚀"
          simple={`Watches for breakouts — when a stock suddenly jumps above its recent high price AND trading volume spikes at the same time. High volume on a breakout means lots of people are buying, which often means the move will continue.`}
          example={`If SPY breaks above its highest price in the last 20 bars AND volume is 1.5x higher than average, it says "BUY" — this is a strong breakout signal.`}
        />

        <StrategyExplainer
          name="ML Signal Generator"
          emoji="🤖"
          simple={`Uses machine learning (AI) to find patterns in the data that humans might miss. It was trained on historical data and looks at dozens of indicators at once to predict whether a stock will go up, down, or stay flat. It only acts when it is at least 65% confident.`}
          example={`The AI model analyzes RSI, MACD, volume, price momentum, and other factors together. If it predicts a stock will rise at least 0.3% with 70% confidence, it says "BUY."`}
        />
      </div>
    </>
  );
}

function StrategyExplainer({
  name,
  emoji,
  simple,
  example,
}: {
  name: string;
  emoji: string;
  simple: string;
  example: string;
}) {
  return (
    <div className="border border-gray-200 rounded-lg p-4">
      <h4 className="font-semibold text-gray-900 mb-2">
        {emoji} {name}
      </h4>
      <p className="text-sm text-gray-700 mb-2">{simple}</p>
      <p className="text-xs text-gray-500 italic">Example: {example}</p>
    </div>
  );
}

function PagesSection() {
  return (
    <>
      <SectionTitle>Understanding Each Dashboard Page</SectionTitle>

      <div className="space-y-4">
        <PageExplainer
          name="Overview ( / )"
          description="Your home base. Shows your account balance, a price chart for any stock you select, your currently open positions (stocks you own), and a live feed of system events. This is where you get the big picture at a glance."
        />
        <PageExplainer
          name="Positions"
          description="Shows every stock you currently hold. For each one, you can see how many shares you own, what you paid, what it's worth now, and whether you're making or losing money on it (shown in green or red). Updates automatically when trades happen."
        />
        <PageExplainer
          name="Orders"
          description="A history of every trade the system has made. Shows what was bought or sold, at what price, whether the order was filled (completed) or rejected, and which strategy triggered it. Use the Refresh button to see the latest orders."
        />
        <PageExplainer
          name="Strategies"
          description="Control center for your four trading strategies. You can turn each one on or off, change their settings (like how sensitive they are), and run backtests — which test the strategy against historical data to see how it would have performed."
        />
        <PageExplainer
          name="Backtest"
          description="Shows the results of backtests — simulated trading on past data. Includes equity curves (charts showing how your money would have grown or shrunk), plus key performance numbers like total return, win rate, and risk metrics. Use the filters to compare strategies and stocks."
        />
        <PageExplainer
          name="Risk Settings"
          description="Where you set the safety guardrails. Control how much you're willing to lose in a day, how big any single position can be, and how many stocks you can hold at once. Also has an emergency halt button that immediately stops all trading."
        />
        <PageExplainer
          name="Logs"
          description="A real-time stream of everything happening in the system: trades being placed, risk rules blocking trades, trading halts, and more. Color-coded by type. Useful for understanding exactly what the system is doing and why."
        />
      </div>
    </>
  );
}

function PageExplainer({ name, description }: { name: string; description: string }) {
  return (
    <div className="border-l-4 border-blue-400 pl-4 py-1">
      <h4 className="font-semibold text-gray-900 text-sm">{name}</h4>
      <p className="text-sm text-gray-600">{description}</p>
    </div>
  );
}

function SafetySection() {
  return (
    <>
      <SectionTitle>Safety & Risk Rules</SectionTitle>
      <P>
        AlgoTrader has built-in safety rules that protect your money. These rules are enforced
        automatically and cannot be bypassed by any strategy. Think of them as guardrails on a
        highway — they keep you from going off the cliff even if the cruise control malfunctions.
      </P>

      <div className="space-y-3">
        <RuleExplainer
          rule="Daily Loss Limit (2%)"
          explanation="If you lose more than 2% of your account value in a single day, ALL trading stops immediately for the rest of the day. This prevents a bad day from becoming a catastrophic day."
        />
        <RuleExplainer
          rule="Position Size Limit (10%)"
          explanation="No single stock can be more than 10% of your total account value. This prevents you from putting too many eggs in one basket."
        />
        <RuleExplainer
          rule="Maximum 4 Open Positions"
          explanation="You can hold at most 4 different stocks at the same time. This forces diversification and limits your exposure."
        />
        <RuleExplainer
          rule="Minimum 60% Confidence"
          explanation="A strategy must be at least 60% confident in its signal before a trade will be placed. Low-confidence guesses are ignored."
        />
        <RuleExplainer
          rule="Order Throttle (5 minutes)"
          explanation="The system can only trade the same stock once every 5 minutes. This prevents rapid-fire trading that could rack up losses."
        />
        <RuleExplainer
          rule="End-of-Day Auto-Close (3:45 PM ET)"
          explanation="All positions are automatically sold at 3:45 PM Eastern, 15 minutes before the market closes. This means you never hold stocks overnight, avoiding surprises from after-hours news."
        />
      </div>

      <div className="bg-red-50 border border-red-200 rounded-lg p-4 text-sm text-red-800 mt-4">
        <strong>Emergency Halt:</strong> If anything goes wrong, go to the Risk Settings page and
        hit the red &quot;Halt Trading&quot; button. This immediately stops all new orders. Your existing
        positions stay open — you would need to sell those manually through Alpaca if needed.
      </div>
    </>
  );
}

function RuleExplainer({ rule, explanation }: { rule: string; explanation: string }) {
  return (
    <div className="flex gap-3 items-start">
      <span className="shrink-0 w-5 h-5 rounded-full bg-green-100 text-green-600 flex items-center justify-center text-xs mt-0.5">
        ✓
      </span>
      <div>
        <span className="text-sm font-medium text-gray-900">{rule}</span>
        <p className="text-sm text-gray-600">{explanation}</p>
      </div>
    </div>
  );
}

function GlossarySection() {
  return (
    <>
      <SectionTitle>Glossary</SectionTitle>
      <P>Quick reference for terms you will see throughout the dashboard.</P>

      <div className="space-y-0 divide-y divide-gray-100">
        <Term term="Bar" definition="A snapshot of a stock's price over a time period (e.g., 5 minutes). Includes the opening price, highest price, lowest price, closing price, and volume (number of shares traded)." />
        <Term term="Backtest" definition="Running a strategy against historical data to see how it would have performed in the past. Not a guarantee of future results, but useful for comparing strategies." />
        <Term term="Bollinger Bands" definition="Lines drawn above and below a stock's average price. When the price touches the lower band, it may be oversold. When it touches the upper band, it may be overbought. The '%B' value tells you where the price is between the bands (0 = lower band, 1 = upper band)." />
        <Term term="Buying Power" definition="How much money you have available to buy stocks. In a margin account, this can be more than your cash balance." />
        <Term term="Confidence" definition="How sure a strategy is about its signal, from 0% (no idea) to 100% (very sure). Signals below 60% confidence are automatically rejected." />
        <Term term="Drawdown (Max DD)" definition="The biggest peak-to-valley drop in your account value. If your account went from $10,000 to $9,000, that's a 10% drawdown. Lower is better." />
        <Term term="Equity" definition="The total value of your account — cash plus the current value of all stocks you hold." />
        <Term term="ETF" definition="Exchange-Traded Fund. A basket of stocks bundled together and traded as a single ticker. SPY tracks the S&P 500 (500 largest US companies). QQQ tracks the Nasdaq 100 (100 largest tech companies)." />
        <Term term="Fill / Filled" definition="When your order actually executes. You place an order to buy, and when someone sells to you, the order is 'filled.' The fill price is what you actually paid." />
        <Term term="HOLD" definition="A signal from a strategy that says 'do nothing right now.' No trade is placed." />
        <Term term="MACD" definition="Moving Average Convergence Divergence. A momentum indicator that shows the relationship between two moving averages. When the MACD line crosses above the signal line, it can indicate upward momentum." />
        <Term term="Paper Trading" definition="Simulated trading with fake money. Everything works exactly like real trading, but no real money is at risk. Always paper trade first before going live." />
        <Term term="P&L (Profit & Loss)" definition="How much money you've made or lost. Green (positive) means profit. Red (negative) means loss. 'Unrealized' P&L is for positions you still hold — it's not locked in until you sell." />
        <Term term="Position" definition="A stock you currently own. If you bought 10 shares of Apple, that's a position in AAPL." />
        <Term term="Profit Factor" definition="Total winning trades divided by total losing trades. Above 1.0 means you're making more than you're losing. Above 1.5 is generally good." />
        <Term term="RSI (Relative Strength Index)" definition="A number from 0 to 100 measuring how fast a stock's price has been rising or falling. Below 30 means 'oversold' (may bounce up). Above 70 means 'overbought' (may pull back). The sweet spot is around 50." />
        <Term term="Sharpe Ratio" definition="A score that measures how good the returns are compared to the risk taken. It answers: 'am I being rewarded enough for the risk?' Above 1.0 is decent, above 2.0 is very good." />
        <Term term="Signal" definition="A recommendation from a strategy: BUY (purchase shares), SELL (sell shares you own), or HOLD (do nothing)." />
        <Term term="Slippage" definition="The difference between the price you expected and the price you actually got. If you tried to buy at $100 but got filled at $100.05, that's $0.05 of slippage." />
        <Term term="SMA (Simple Moving Average)" definition="The average closing price over a set number of bars. A 10-bar SMA averages the last 10 closing prices. It smooths out noise and shows the general trend direction." />
        <Term term="SSE (Server-Sent Events)" definition="The technology that makes the dashboard update in real time without refreshing the page. The server pushes events (trades, position updates, etc.) to your browser as they happen." />
        <Term term="Volume" definition="The number of shares traded during a time period. High volume often confirms that a price move is significant — many people agree on the direction." />
        <Term term="Win Rate" definition="The percentage of trades that made money. A 60% win rate means 6 out of 10 trades were profitable. Even 50-55% can be good if your winners are bigger than your losers." />
      </div>
    </>
  );
}

function Term({ term, definition }: { term: string; definition: string }) {
  return (
    <div className="py-3">
      <dt className="text-sm font-semibold text-gray-900">{term}</dt>
      <dd className="text-sm text-gray-600 mt-0.5">{definition}</dd>
    </div>
  );
}
