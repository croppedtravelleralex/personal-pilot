import { useMemo, useState } from "react";

import {
  collectValidationReport,
  exportValidationProfileEvidence,
  listValidationReports,
} from "../../services/desktop";
import type {
  DesktopValidationProfileExport,
  DesktopValidationReport,
  DesktopValidationReportSummary,
} from "../../types/desktop";
import {
  buildValidationBoardSnapshot,
  summarizeValidationBoard,
} from "./model";

export function useValidationBoardViewModel() {
  const [report, setReport] = useState<DesktopValidationReport | null>(null);
  const [history, setHistory] = useState<DesktopValidationReportSummary[]>([]);
  const [lastExport, setLastExport] = useState<DesktopValidationProfileExport | null>(null);
  const [isCollecting, setIsCollecting] = useState(false);
  const [isLoadingHistory, setIsLoadingHistory] = useState(false);
  const [isExporting, setIsExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const snapshot = useMemo(() => buildValidationBoardSnapshot(report), [report]);
  const summary = useMemo(() => summarizeValidationBoard(snapshot), [snapshot]);

  async function refreshHistory() {
    setIsLoadingHistory(true);
    setError(null);
    try {
      setHistory(await listValidationReports());
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Validation report history could not be loaded.",
      );
    } finally {
      setIsLoadingHistory(false);
    }
  }

  async function collectObservedEvidence() {
    setIsCollecting(true);
    setError(null);
    try {
      const nextReport = await collectValidationReport();
      setReport(nextReport);
      setHistory(await listValidationReports());
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

  async function exportProfileEvidence(profileId?: string | null) {
    setIsExporting(true);
    setError(null);
    try {
      setLastExport(await exportValidationProfileEvidence(profileId));
    } catch (nextError) {
      setError(
        nextError instanceof Error
          ? nextError.message
          : "Validation profile evidence export failed.",
      );
    } finally {
      setIsExporting(false);
    }
  }

  return {
    collectObservedEvidence,
    error,
    exportProfileEvidence,
    history,
    isCollecting,
    isExporting,
    isLoadingHistory,
    lastExport,
    refreshHistory,
    report,
    snapshot,
    summary,
  };
}
