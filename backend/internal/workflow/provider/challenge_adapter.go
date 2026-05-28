package provider

import (
	"context"
	"personal-pilot/backend/internal/captcha"
)

type ChallengeAdapter struct {
	Manager *captcha.Manager
}

func (a ChallengeAdapter) Call(ctx context.Context, request Request) (Response, error) {
	result, err := a.Manager.Solve(ctx, &captcha.SolveRequest{Type: captcha.CaptchaType(request.Payload["type"]), SiteKey: request.Payload["siteKey"], PageURL: request.Payload["pageUrl"]})
	if err != nil {
		return Response{}, err
	}
	return Response{Status: "solved", Payload: map[string]string{"token": result.Token, "text": result.Text, "provider": result.Provider}}, nil
}
