// Undo: shows what undoing the latest restore would put back, then does it.

import { useEffect, useState } from "react";
import type { Go } from "../App";
import { errorText, previewUndo, undo, type UndoPreview } from "../api";
import { Loading, Notice, QuitApps, Screen } from "../components";
import { count, when } from "../format";

export default function Undo(props: { go: Go }) {
  const [preview, setPreview] = useState<UndoPreview | null | undefined>(undefined);
  const [error, setError] = useState<string | null>(null);
  const [working, setWorking] = useState(false);
  const [done, setDone] = useState(false);

  const load = () => {
    previewUndo().then(setPreview, (err) => {
      setPreview(null);
      setError(errorText(err));
    });
  };
  useEffect(load, []);

  const onUndo = async () => {
    if (!preview) return;
    setError(null);
    setWorking(true);
    try {
      await undo(preview.id);
      setDone(true);
    } catch (err) {
      setError(errorText(err));
      load();
    } finally {
      setWorking(false);
    }
  };

  const home = () => props.go({ name: "home" });

  if (done) {
    return (
      <Screen title="Undone">
        <Notice tone="success">Your earlier settings are back, just as they were.</Notice>
        <button className="primary" onClick={home}>
          Done
        </button>
      </Screen>
    );
  }

  if (preview === undefined) {
    return (
      <Screen title="Undo" onBack={home}>
        <Loading>Looking for the last restore…</Loading>
      </Screen>
    );
  }

  if (preview === null) {
    return (
      <Screen title="Undo" onBack={home}>
        {error ? (
          <Notice tone="error" title="Couldn't check">{error}</Notice>
        ) : (
          <p className="lead">There's nothing to undo. No settings have been put on this computer yet, or the last time was already undone.</p>
        )}
      </Screen>
    );
  }

  const putBack = preview.steps.filter((step) => step.kind === "put_back");
  const removed = preview.steps.filter((step) => step.kind === "delete");

  return (
    <Screen
      title="Undo the last restore?"
      intro={
        <>
          Settings from <strong>{preview.bundle_name}</strong> were put on this computer on{" "}
          {when(preview.created)}. Undoing puts everything back the way it was before.
        </>
      }
      onBack={home}
      footer={
        <button
          className="primary"
          disabled={working || preview.running.length > 0}
          onClick={onUndo}
        >
          {working ? "Undoing…" : "Undo restore"}
        </button>
      }
    >
      {error && <Notice tone="error" title="Nothing was undone">{error}</Notice>}
      {preview.running.length > 0 && (
        <QuitApps apps={preview.running} doing="putting things back" onCheck={load} />
      )}
      {putBack.length > 0 && (
        <section>
          <h2>Your earlier version comes back ({count(putBack.length, "file")})</h2>
          <StepList steps={putBack} />
        </section>
      )}
      {removed.length > 0 && (
        <section>
          <h2>Removed, since they weren't here before ({count(removed.length, "file")})</h2>
          <StepList steps={removed} />
        </section>
      )}
    </Screen>
  );
}

function StepList(props: { steps: UndoPreview["steps"] }) {
  return (
    <ul className="file-list">
      {props.steps.map((step) => (
        <li key={step.path}>
          <strong>{step.app_name}</strong>{" "}
          <span className="file-path muted small">{step.path}</span>
        </li>
      ))}
    </ul>
  );
}
