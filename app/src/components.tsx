// Pieces shared by the screens.

import { useState, type ReactNode } from "react";
import type { Diff } from "./api";
import { quoted, size } from "./format";

export function Screen(props: {
  title: string;
  intro?: ReactNode;
  onBack?: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  return (
    <main className="screen">
      <header className="screen-header">
        {props.onBack && (
          <button className="link back" onClick={props.onBack}>
            ← Back
          </button>
        )}
        <h1>{props.title}</h1>
        {props.intro && <p className="intro">{props.intro}</p>}
      </header>
      <div className="screen-body">{props.children}</div>
      {props.footer && <footer className="screen-footer">{props.footer}</footer>}
    </main>
  );
}

/** Where the user is in a flow of several screens. */
export function Steps(props: { steps: string[]; current: number }) {
  return (
    <ol className="steps" aria-label="Progress">
      {props.steps.map((step, i) => (
        <li
          key={step}
          className={i === props.current ? "current" : i < props.current ? "done" : ""}
          aria-current={i === props.current ? "step" : undefined}
        >
          {step}
        </li>
      ))}
    </ol>
  );
}

export function Loading(props: { children: ReactNode }) {
  return (
    <div className="loading" role="status">
      <span className="spinner" aria-hidden="true" />
      {props.children}
    </div>
  );
}

export function Notice(props: {
  tone: "info" | "warning" | "error" | "success";
  title?: string;
  children: ReactNode;
}) {
  return (
    <div className={`notice ${props.tone}`} role={props.tone === "error" ? "alert" : undefined}>
      {props.title && <strong className="notice-title">{props.title}</strong>}
      <div>{props.children}</div>
    </div>
  );
}

/** An engine message, with its `quoted` parts shown as code. */
export function Message(props: { text: string }) {
  return (
    <>
      {quoted(props.text).map((part, i) =>
        i % 2 === 1 ? <code key={i}>{part}</code> : <span key={i}>{part}</span>,
      )}
    </>
  );
}

/** What a file will look like after the change, behind a toggle. */
export function DiffView(props: { diff: Diff }) {
  const [open, setOpen] = useState(false);
  const { diff } = props;
  if (diff.kind === "nothing") return null;
  if (diff.kind === "line_endings_only") {
    return <p className="muted small">Only the line endings change.</p>;
  }
  if (diff.kind === "binary") {
    return (
      <p className="muted small">
        Not a text file, so the changes can't be shown (
        {diff.old === null ? size(diff.new) : `${size(diff.old)} → ${size(diff.new)}`}).
      </p>
    );
  }
  return (
    <div className="diff-toggle">
      <button className="link small" onClick={() => setOpen(!open)} aria-expanded={open}>
        {open ? "Hide changes" : "Show changes"}
      </button>
      {open && (
        <pre className="diff" aria-label="Changes to this file">
          {diff.lines.map((line, i) => (
            <div key={i} className={`diff-${line.kind}`}>
              {line.kind === "added" ? "+ " : line.kind === "removed" ? "− " : line.kind === "context" ? "  " : ""}
              {line.text}
            </div>
          ))}
          {diff.hidden > 0 && <div className="diff-hunk">… {diff.hidden} more lines</div>}
        </pre>
      )}
    </div>
  );
}

/** Asks the user to quit apps, with a way to check again. */
export function QuitApps(props: { apps: string[]; doing: string; onCheck: () => void }) {
  return (
    <Notice tone="warning" title={`Please quit ${props.apps.join(" and ")} first`}>
      <p>
        An open app can write over its settings when it quits, which would undo {props.doing}.
      </p>
      <button className="secondary" onClick={props.onCheck}>
        I've quit, check again
      </button>
    </Notice>
  );
}
