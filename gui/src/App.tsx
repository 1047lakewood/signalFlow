import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";

function App() {
  const [status, setStatus] = useState<string>("Loading...");

  useEffect(() => {
    invoke<string>("get_status").then(setStatus).catch(console.error);
  }, []);

  return (
    <div className="app">
      <header className="header">
        <h1>signalFlow</h1>
      </header>
      <main className="main">
        <pre className="status">{status}</pre>
      </main>
    </div>
  );
}

export default App;
