# CompFin: Enterprise Treasury & Quantitative Pricing System

[![Language: Rust](https://img.shields.io/badge/Language-Rust-blue.svg)](https://www.rust-lang.org/)
[![Status: Active](https://img.shields.io/badge/Status-Active-success.svg)]()

CompFin 是一個以 Rust 打造的高效能資金管理系統（Treasury System）與量化定價引擎。本專案專注於提供機構級的市場數據管理、衍生性商品定價以及合規風險計算，並嚴格遵循 idiomatic Rust 的開發準則，確保系統的極致效能與記憶體安全。

## 核心架構與功能 (Core Features)

* **量化定價引擎 (Quantitative Pricing Engine):** 實作高精度的金融數學模型，支援利率衍生性商品（包含 Swaption）的定價與風險計算。
* **市場數據管理 (Market Data Management):** 具備完整的市場結構定義，涵蓋單幣別市場（SingleCurrencyMarket）與 FX 市場（FxMarket），並支援多種結算慣例（Settlement Convention）與假日 Calendar 管理。
* **排程與日期生成 (Schedule & Date Generation):** 內建完整的 Schedule 生成器，支援各種 Stub 慣例、EOM 規則、Generation Direction，以及 Option Expiry / Delivery 日期的產生（OptionDateGenerator），符合 Clark (2012) 的市場慣例。
* **風險與合規 (Risk & Compliance):** 內建對應現代金融監管規範（如 CRR3）的邏輯計算與風險建模能力。
* **利率指數快取 (Interest Rate Index Cache):** 提供可插拔的快取後端（CacheBackend），支援利率指數的高效查詢與狀態管理。

## 技術棧 (Tech Stack)

* **核心語言:** Rust
* **開發環境:** VS Code


## 專案亮點與 AI 協作 (AI Collaboration)

本系統在具備極高領域專業性（Domain Knowledge）與高精度邏輯要求的背景下開發。程式碼的演算法精確度與架構設計，深度結合了目前最先進的 AI 輔助開發模型：

> 💡 **Special Thanks:**
> 本專案的核心邏輯、架構設計與程式碼實作，是由開發者與 **Claude Sonnet 4.6 Extended Thinking** 以及 **Gemini Pro 3.1** 共同深度協作完成的。這兩大模型的協助，大幅提升了開發效率與量化邏輯的精準度。

## 安裝與執行 (Getting Started)

*(請確保您的開發環境已安裝最新版的 Rust 工具鏈)*

```bash
# 複製專案
git clone https://github.com/gfunk0704/CompFin.git
cd CompFin

# 安裝依賴並編譯
cargo build --release

# 執行範例（需先設定 JSON 設定檔路徑）
cargo run --release
```

> ⚠️ 執行前請確認 `src/main.rs` 內的 `JSON_FOLDER` 路徑已更新為本機的設定檔目錄。