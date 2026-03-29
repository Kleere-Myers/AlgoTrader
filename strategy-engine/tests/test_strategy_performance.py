"""Tests for GET /strategies/{id}/performance endpoint."""

import sqlite3
from datetime import datetime, timezone, timedelta
from unittest.mock import patch

import pytest
from fastapi.testclient import TestClient


@pytest.fixture
def _mock_db(tmp_path):
    """Create a test SQLite DB with signals table and seed data."""
    db_path = str(tmp_path / "test.sqlite")
    con = sqlite3.connect(db_path)
    con.execute(
        "CREATE TABLE signals ("
        "  id INTEGER PRIMARY KEY, strategy_name TEXT, symbol TEXT,"
        "  timestamp TEXT, direction TEXT, confidence REAL, reason TEXT"
        ")"
    )

    now = datetime.now(timezone.utc)
    recent = (now - timedelta(days=5)).isoformat()
    old = (now - timedelta(days=60)).isoformat()

    rows = [
        # Recent signals (within 30 days) for RSIMeanReversion
        (1, "RSIMeanReversion", "AAPL", recent, "BUY", 0.72, "rsi low"),
        (2, "RSIMeanReversion", "AAPL", recent, "SELL", 0.68, "rsi high"),
        (3, "RSIMeanReversion", "SPY", recent, "BUY", 0.80, "rsi low"),
        (4, "RSIMeanReversion", "SPY", recent, "HOLD", 0.0, "no cross"),
        (5, "RSIMeanReversion", "MSFT", recent, "HOLD", 0.0, "no cross"),
        # Old signal — should NOT be included
        (6, "RSIMeanReversion", "AAPL", old, "BUY", 0.90, "old signal"),
        # Different strategy — should NOT be included
        (7, "MomentumVolume", "AAPL", recent, "BUY", 0.55, "breakout"),
    ]

    for r in rows:
        con.execute("INSERT INTO signals VALUES (?, ?, ?, ?, ?, ?, ?)", list(r))
    con.commit()
    con.close()

    with patch("main.DB_PATH", db_path):
        yield


@pytest.fixture
def client(_mock_db):
    from main import app
    return TestClient(app)


class TestStrategyPerformance:
    def test_returns_correct_counts(self, client):
        resp = client.get("/strategies/RSIMeanReversion/performance")
        assert resp.status_code == 200
        data = resp.json()
        assert data["strategy_name"] == "RSIMeanReversion"
        assert data["total_signals"] == 5
        assert data["buy_signals"] == 2
        assert data["sell_signals"] == 1
        assert data["hold_signals"] == 2

    def test_avg_confidence(self, client):
        resp = client.get("/strategies/RSIMeanReversion/performance")
        data = resp.json()
        # (0.72 + 0.68 + 0.80 + 0.0 + 0.0) / 5 = 0.44
        assert data["avg_confidence"] == pytest.approx(0.44, abs=0.001)

    def test_signals_by_symbol(self, client):
        resp = client.get("/strategies/RSIMeanReversion/performance")
        data = resp.json()
        assert data["signals_by_symbol"] == {"AAPL": 2, "MSFT": 1, "SPY": 2}

    def test_unknown_strategy_404(self, client):
        resp = client.get("/strategies/DoesNotExist/performance")
        assert resp.status_code == 404

    def test_strategy_with_no_recent_signals(self, client):
        resp = client.get("/strategies/MovingAverageCrossover/performance")
        assert resp.status_code == 200
        data = resp.json()
        assert data["total_signals"] == 0
        assert data["buy_signals"] == 0
        assert data["avg_confidence"] == 0.0
        assert data["signals_by_symbol"] == {}
