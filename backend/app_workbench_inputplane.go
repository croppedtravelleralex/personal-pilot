package backend

import (
	"fmt"
	"strings"

	"personal-pilot/backend/internal/behavior"
	"personal-pilot/backend/internal/behavior/inputplane"
	"personal-pilot/backend/internal/launchcode"
)

func (a *App) resolveInputPlane(action *launchcode.ActionRequest) inputplane.Plane {
	if action == nil {
		return inputplane.PlaneCDP
	}
	mode := inputplane.ParseMode(action.InputMode)
	hasFrame := strings.TrimSpace(action.Frame) != ""
	return inputplane.ResolvePlane(mode, action.Type, hasFrame, inputplane.OSAvailable())
}

func (a *App) prepareOSInput(profile *BrowserProfile, plane inputplane.Plane) error {
	if plane != inputplane.PlaneOS || profile == nil {
		return nil
	}
	if profile.Pid <= 0 {
		return fmt.Errorf("os input: browser pid not ready")
	}
	if err := activateExternalWindowByPID(profile.Pid); err != nil {
		return fmt.Errorf("os input: activate window: %w", err)
	}
	return nil
}

func (a *App) dispatchClickAction(
	executor *behavior.CDPExecutor,
	profile *BrowserProfile,
	action *launchcode.ActionRequest,
	result *launchcode.ActionResult,
) error {
	if action.Selector == "" {
		return fmt.Errorf("selector is required for click")
	}
	if action.Frame != "" {
		result.InputPlane = string(inputplane.PlaneCDP)
		return executor.ExecuteHumanizedClickInFrame(action.Frame, action.Selector)
	}

	plane := a.resolveInputPlane(action)
	if plane == inputplane.PlaneOS {
		if err := a.prepareOSInput(profile, plane); err != nil {
			if inputplane.ParseMode(action.InputMode) == inputplane.ModeAuto {
				plane = inputplane.PlaneCDP
			} else {
				return err
			}
		} else if err := inputplane.ExecuteClickWithSeed(plane, executor, profile.Pid, action.Selector, 0, 0, profile.HumanizeSeed); err != nil {
			if inputplane.ParseMode(action.InputMode) == inputplane.ModeAuto {
				plane = inputplane.PlaneCDP
			} else {
				return err
			}
		} else {
			result.InputPlane = string(inputplane.PlaneOS)
			return nil
		}
	}
	result.InputPlane = string(inputplane.PlaneCDP)
	return executor.ExecuteHumanizedClick(action.Selector)
}

func (a *App) dispatchTypeAction(
	executor *behavior.CDPExecutor,
	profile *BrowserProfile,
	action *launchcode.ActionRequest,
	result *launchcode.ActionResult,
) error {
	if action.Selector == "" {
		return fmt.Errorf("selector is required for type")
	}
	clearFirst := true
	if action.ClearFirst != nil {
		clearFirst = *action.ClearFirst
	}
	if action.Frame != "" {
		result.InputPlane = string(inputplane.PlaneCDP)
		return executor.ExecuteHumanizedTypeInFrame(action.Frame, action.Selector, action.Text, clearFirst, action.SubmitOnEnter)
	}

	plane := a.resolveInputPlane(action)
	if plane == inputplane.PlaneOS {
		if err := a.prepareOSInput(profile, plane); err != nil {
			if inputplane.ParseMode(action.InputMode) == inputplane.ModeAuto {
				plane = inputplane.PlaneCDP
			} else {
				return err
			}
		} else if err := inputplane.ExecuteTypeWithSeed(plane, executor, profile.Pid, action.Selector, action.Text, clearFirst, action.SubmitOnEnter, profile.HumanizeSeed); err != nil {
			if inputplane.ParseMode(action.InputMode) == inputplane.ModeAuto {
				plane = inputplane.PlaneCDP
			} else {
				return err
			}
		} else {
			result.InputPlane = string(inputplane.PlaneOS)
			return nil
		}
	}
	result.InputPlane = string(inputplane.PlaneCDP)
	return executor.ExecuteHumanizedTypeEx(action.Selector, action.Text, clearFirst, action.SubmitOnEnter)
}

func (a *App) dispatchDoubleClickAction(
	executor *behavior.CDPExecutor,
	profile *BrowserProfile,
	action *launchcode.ActionRequest,
	result *launchcode.ActionResult,
) error {
	if action.Selector == "" {
		return fmt.Errorf("selector is required for double-click")
	}
	if action.Frame != "" {
		result.InputPlane = string(inputplane.PlaneCDP)
		return executor.DoubleClickElementInFrame(action.Frame, action.Selector)
	}

	plane := a.resolveInputPlane(action)
	if plane == inputplane.PlaneOS {
		if err := a.prepareOSInput(profile, plane); err != nil {
			if inputplane.ParseMode(action.InputMode) != inputplane.ModeAuto {
				return err
			}
			plane = inputplane.PlaneCDP
		} else if err := inputplane.ExecuteDoubleClickWithSeed(plane, executor, profile.Pid, action.Selector, profile.HumanizeSeed); err != nil {
			if inputplane.ParseMode(action.InputMode) != inputplane.ModeAuto {
				return err
			}
			plane = inputplane.PlaneCDP
		} else {
			result.InputPlane = string(inputplane.PlaneOS)
			return nil
		}
	}
	result.InputPlane = string(inputplane.PlaneCDP)
	return executor.DoubleClickElement(action.Selector)
}

func (a *App) dispatchRightClickAction(
	executor *behavior.CDPExecutor,
	profile *BrowserProfile,
	action *launchcode.ActionRequest,
	result *launchcode.ActionResult,
) error {
	if action.Selector == "" {
		return fmt.Errorf("selector is required for right-click")
	}
	if action.Frame != "" {
		result.InputPlane = string(inputplane.PlaneCDP)
		return executor.RightClickElementInFrame(action.Frame, action.Selector)
	}

	plane := a.resolveInputPlane(action)
	if plane == inputplane.PlaneOS {
		if err := a.prepareOSInput(profile, plane); err != nil {
			if inputplane.ParseMode(action.InputMode) != inputplane.ModeAuto {
				return err
			}
			plane = inputplane.PlaneCDP
		} else if err := inputplane.ExecuteRightClickWithSeed(plane, executor, profile.Pid, action.Selector, profile.HumanizeSeed); err != nil {
			if inputplane.ParseMode(action.InputMode) != inputplane.ModeAuto {
				return err
			}
			plane = inputplane.PlaneCDP
		} else {
			result.InputPlane = string(inputplane.PlaneOS)
			return nil
		}
	}
	result.InputPlane = string(inputplane.PlaneCDP)
	return executor.RightClickElement(action.Selector)
}

func (a *App) dispatchClickOffsetAction(
	executor *behavior.CDPExecutor,
	profile *BrowserProfile,
	action *launchcode.ActionRequest,
	result *launchcode.ActionResult,
) error {
	if action.Selector == "" {
		return fmt.Errorf("selector is required for click-offset")
	}
	offsetX, offsetY := 0, 0
	if action.OffsetX != nil {
		offsetX = *action.OffsetX
	}
	if action.OffsetY != nil {
		offsetY = *action.OffsetY
	}
	if action.Frame != "" {
		result.InputPlane = string(inputplane.PlaneCDP)
		return executor.ClickWithOffsetInFrame(action.Frame, action.Selector, offsetX, offsetY)
	}

	plane := a.resolveInputPlane(action)
	if plane == inputplane.PlaneOS {
		if err := a.prepareOSInput(profile, plane); err != nil {
			if inputplane.ParseMode(action.InputMode) != inputplane.ModeAuto {
				return err
			}
			plane = inputplane.PlaneCDP
		} else if err := inputplane.ExecuteClickWithSeed(plane, executor, profile.Pid, action.Selector, offsetX, offsetY, profile.HumanizeSeed); err != nil {
			if inputplane.ParseMode(action.InputMode) != inputplane.ModeAuto {
				return err
			}
			plane = inputplane.PlaneCDP
		} else {
			result.InputPlane = string(inputplane.PlaneOS)
			return nil
		}
	}
	result.InputPlane = string(inputplane.PlaneCDP)
	return executor.ClickWithOffset(action.Selector, offsetX, offsetY)
}
