# EchoMon — Real-time ICMP echo monitoring tool.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Tauri](https://img.shields.io/badge/built%20with-Tauri-24C8DB.svg)
![Rust](https://img.shields.io/badge/backend-Rust-orange.svg)

A modern, high-performance network ping tool built with **Tauri (Rust)** and **TypeScript**. Designed for network administrators and gamers who need accurate, real-time latency monitoring with historical data tracking.

## 🚀 Features

*   **Real-Time Monitoring:** accurate ICMP ping latency (RTT) with live updates.
*   **Visual Graphing:** Live line chart visualization of network performance.
*   **Session History:** Automatically saves every session to a local SQLite database (`ping_history.db`).
*   **Detailed Statistics:** Tracks Min, Max, Avg RTT, Packet Loss %, and Uptime.
*   **Export Capabilities:**
    *   📄 **PDF Reports:** Generate professional reports with embedded charts.
    *   📋 **TXT Logs:** Export raw ping logs for analysis.
*   **Dark Mode UI:** sleek, cyber-inspired interface using Tailwind CSS.
*   **Cross-Platform:** Runs on Windows, macOS, and Linux.

## 🛠️ Tech Stack

*   **Frontend:** HTML5, TypeScript, Tailwind CSS
*   **Backend:** Rust (Tauri v2)
*   **Database:** SQLite (via `rusqlite`)
*   **Visualization:** Chart.js
*   **PDF Generation:** jsPDF

## 📦 Installation & Usage

### Prerequisites
*   [Node.js](https://nodejs.org/) (v16+)
*   [Rust](https://www.rust-lang.org/tools/install) (latest stable)

### 1. Clone the Repository
```bash
git clone https://github.com/var2611/EchoMon.git
cd echomon
```

### 2. Install Dependencies
```bash
npm install
```

### 3. Run in Development Mode
```bash
npm run tauri dev
```

### 4. Build for Production
To create a standalone executable (single `.exe` or `.app`):
```bash
npm run tauri build
```
The output binary will be located in `src-tauri/target/release/bundle/`.

## 📝 License

This project is open-source and available under the **MIT License**.

You are free to use, modify, and distribute this software for personal or commercial purposes.
**Attribution is appreciated but not required.** If you use this code in your own projects, a shoutout or link back to this repository would be awesome!

```text
MIT License

Copyright (c) 2024 EchoMon Contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
```

## 🤝 Contributing

Contributions are welcome! Feel free to open an issue or submit a pull request if you have ideas for improvements.

1.  Fork the Project
2.  Create your Feature Branch (`git checkout -b feature/AmazingFeature`)
3.  Commit your Changes (`git commit -m 'Add some AmazingFeature'`)
4.  Push to the Branch (`git push origin feature/AmazingFeature`)
5.  Open a Pull Request
