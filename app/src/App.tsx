// The app is two short journeys: save settings on the old computer (Scan,
// Choose apps, Capture) and put them on the new one (Restore preview), plus
// Undo. Each screen writes nothing until its final button.

import { useEffect, useState } from "react";
import { appInfo, type AppInfo, type ScannedApp } from "./api";
import Home from "./screens/Home";
import Scan from "./screens/Scan";
import ChooseApps from "./screens/ChooseApps";
import Capture from "./screens/Capture";
import RestorePreview from "./screens/RestorePreview";
import Undo from "./screens/Undo";

export type Route =
  | { name: "home" }
  | { name: "scan" }
  | { name: "choose"; apps: ScannedApp[]; chosen?: string[] }
  | { name: "capture"; apps: ScannedApp[]; chosen: string[] }
  | { name: "restore" }
  | { name: "undo" };

export type Go = (route: Route) => void;

export default function App() {
  const [route, setRoute] = useState<Route>({ name: "home" });
  const [info, setInfo] = useState<AppInfo | null>(null);

  useEffect(() => {
    appInfo().then(setInfo, () => setInfo(null));
  }, []);

  const go: Go = (next) => {
    setRoute(next);
    window.scrollTo(0, 0);
  };

  return (
    <div className="app">
      {info?.test_home && (
        <div className="test-banner">
          Test mode: using {info.test_home} instead of your real home folder.
        </div>
      )}
      {route.name === "home" && <Home go={go} os={info?.os} />}
      {route.name === "scan" && <Scan go={go} />}
      {route.name === "choose" && <ChooseApps go={go} apps={route.apps} chosen={route.chosen} />}
      {route.name === "capture" && <Capture go={go} apps={route.apps} chosen={route.chosen} />}
      {route.name === "restore" && <RestorePreview go={go} />}
      {route.name === "undo" && <Undo go={go} />}
    </div>
  );
}
