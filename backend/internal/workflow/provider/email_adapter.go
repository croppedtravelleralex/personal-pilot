package provider

import (
	"context"
	"personal-pilot/backend/internal/email"
)

type EmailAdapter struct {
	Service *email.EmailService
}

func (a EmailAdapter) Call(ctx context.Context, request Request) (Response, error) {
	if request.Kind != "email:create_inbox" {
		return Response{Status: "unsupported", Payload: map[string]string{"kind": request.Kind}}, nil
	}
	session, err := a.Service.CreateInbox(ctx)
	if err != nil {
		return Response{}, err
	}
	return Response{Status: session.Status, Payload: map[string]string{"id": session.ID, "email": session.Email, "provider": session.Provider}}, nil
}
