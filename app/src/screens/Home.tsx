import type { Go } from "../App";

export default function Home(props: { go: Go; os?: string }) {
  return (
    <main className="screen home">
      <header className="screen-header">
        <h1>Move your app settings</h1>
        <p className="intro">
          Take your settings from one computer to another, even between Windows and Mac.
          Passwords and sign-ins are never copied.
        </p>
      </header>
      <div className="choices">
        <button className="choice" onClick={() => props.go({ name: "scan" })}>
          <span className="choice-icon" aria-hidden="true">⬆</span>
          <span className="choice-title">Save settings from this computer</span>
          <span className="choice-text">
            Use this on your old computer. You'll get one file to copy to the new one.
          </span>
        </button>
        <button className="choice" onClick={() => props.go({ name: "restore" })}>
          <span className="choice-icon" aria-hidden="true">⬇</span>
          <span className="choice-title">Put settings on this computer</span>
          <span className="choice-text">
            Use this on your new computer, with the file you saved. You'll see every change
            before anything happens.
          </span>
        </button>
      </div>
      <footer className="home-footer">
        <button className="link" onClick={() => props.go({ name: "undo" })}>
          Undo the last time settings were put on this computer
        </button>
        {props.os && <span className="muted small">This computer runs {props.os}.</span>}
      </footer>
    </main>
  );
}
