import { useMemo, useState } from "react";

import { collectValidationReport } from "../../services/desktop";
import type { DesktopValidationReport } from "../../types/desktop";
import {
  buildValidationBoardSnapshot,
  summarizeValidationBoard,
} from "./model";

export function useValidationBoardViewModel() {
  const [report, setReport] = useState<DesktopValidationReport | null>(null);
  const [isCollecting, setIsCollecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const snapshot = useMemo(() => buildValidationBoardSnapshot(report), [report]);
  const summary = useMemo(() => summarizeValidationBoard(snapshot), [snapshot]);

  async function collectObservedEvidence() {
    setIsCollecting(true);
    setError(null);
    try {
      const nextReport = await collectValidationReport();
      setReport(nextReport);
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Validation evidence collection failed.",
      );
    } finally {
      setIsCollecting(false);
    }
  }

  return {
    collectObservedEvidence,
    error,
    isCollecting,
    report,
    snapshot,
    summary,
  };
}
