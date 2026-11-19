import { testHome } from "../guard";

/** Runs once before any test: the safety guard. */
export default function globalSetup() {
  testHome();
}
