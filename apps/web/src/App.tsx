import { useEffect, useState } from 'react';
import './App.css';
import reactLogo from './assets/react.svg';
import viteLogo from '/vite.svg';

function App() {
  const [count, setCount] = useState(0)
  const [greetMsg, setGreetMsg] = useState("")
  const [name, setName] = useState("")
  const [isTauri, setIsTauri] = useState(false)

  useEffect(() => {
    setIsTauri(!!window.__TAURI__)
  }, [])

  async function greet() {
    try {
      if (window.__TAURI__) {
        const { invoke } = await import("@tauri-apps/api/core");
        setGreetMsg(await invoke("greet", { name }));
      } else {
        setGreetMsg("Running in web mode - Tauri not available");
      }
    } catch (e) {
      setGreetMsg("Tauri command failed: " + e);
    }
  }

  return (
    <>
      <div>
        <a href="https://vite.dev" target="_blank">
          <img src={viteLogo} className="logo" alt="Vite logo" />
        </a>
        <a href="https://react.dev" target="_blank">
          <img src={reactLogo} className="logo react" alt="React logo" />
        </a>
      </div>
      <h1>Vite + React {isTauri ? '(Tauri)' : '(Web)'}</h1>

      {isTauri && (
        <div className="card">
          <input
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder="Enter a name..."
          />
          <button onClick={greet}>
            Greet
          </button>
          {greetMsg && <p>{greetMsg}</p>}
        </div>
      )}

      <div className="card">
        <button onClick={() => setCount((count) => count + 1)}>
          count is {count}
        </button>
        <p>
          Edit <code>src/App.tsx</code> and save to test HMR
        </p>
      </div>
      <p className="read-the-docs">
        Click on the Vite and React logos to learn more
      </p>
    </>
  )
}

export default App
