import { useMemo } from "react";

import {
  buildValidationBoardSnapshot,
  summarizeValidationBoard,
} from "./model";

export function useValidationBoardViewModel() {
  return useMemo(() => {
    const snapshot = buildValidationBoardSnapshot();
    return {
      snapshot,
      summary: summarizeValidationBoard(snapshot),
    };
  }, []);
}
