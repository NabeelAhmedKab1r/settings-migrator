import { useState } from "react";
import type { Go } from "../App";
import type { ScannedApp } from "../api";
import { Screen, Steps } from "../components";
import { count, size } from "../format";
import { SAVE_STEPS } from "./Scan";

export default function ChooseApps(props: { go: Go; apps: ScannedApp[]; chosen?: string[] }) {
  const found = props.apps.filter((app) => app.status === "found");
  const [chosen, setChosen] = useState<string[]>(props.chosen ?? found.map((app) => app.id));
  const [open, setOpen] = useState<string | null>(null);

  const toggle = (id: string) =>
    setChosen(chosen.includes(id) ? chosen.filter((c) => c !== id) : [...chosen, id]);
  const all = chosen.length === found.length;

  return (
    <Screen
      title="Which apps should come with you?"
      intro="Their settings go into one file. Nothing is saved until the next step."
      onBack={() => props.go({ name: "scan" })}
      footer={
        <button
          className="primary"
          disabled={chosen.length === 0}
          onClick={() => props.go({ name: "capture", apps: props.apps, chosen })}
        >
          Next: check and save
        </button>
      }
    >
      <Steps steps={SAVE_STEPS} current={1} />
      <button className="link small" onClick={() => setChosen(all ? [] : found.map((app) => app.id))}>
        {all ? "Choose none" : "Choose all"}
      </button>
      <ul className="app-list">
        {found.map((app) => (
          <li key={app.id} className="app-row choose">
            <label className="check">
              <input
                type="checkbox"
                checked={chosen.includes(app.id)}
                onChange={() => toggle(app.id)}
              />
              <span className="app-name">{app.name}</span>
            </label>
            <button
              className="link small"
              aria-expanded={open === app.id}
              onClick={() => setOpen(open === app.id ? null : app.id)}
            >
              {count(app.files.length, "file")}
            </button>
            {open === app.id && (
              <ul className="file-list">
                {app.files.map((file) => (
                  <li key={file.path}>
                    {file.path} <span className="muted">({size(file.size)})</span>
                  </li>
                ))}
              </ul>
            )}
          </li>
        ))}
      </ul>
      <p className="muted small">
        Passwords, sign-ins and private keys are never included, whatever you choose.
      </p>
    </Screen>
  );
}
