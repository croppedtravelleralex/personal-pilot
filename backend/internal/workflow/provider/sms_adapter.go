package provider

import (
	"context"
	"personal-pilot/backend/internal/sms"
)

type SMSAdapter struct {
	Provider sms.Provider
}

func (a SMSAdapter) Call(ctx context.Context, request Request) (Response, error) {
	switch request.Kind {
	case "sms:buy":
		number, err := a.Provider.BuyNumber(ctx, &sms.BuyRequest{Country: request.Payload["country"], Service: request.Payload["service"], Operator: request.Payload["operator"]})
		if err != nil {
			return Response{}, err
		}
		return Response{Status: string(number.Status), Payload: map[string]string{"id": number.ID, "phone": number.Phone}}, nil
	case "sms:check":
		result, err := a.Provider.CheckSMS(ctx, request.Payload["id"])
		if err != nil {
			return Response{}, err
		}
		payload := map[string]string{"status": string(result.Status)}
		if result.SMS != nil {
			payload["code"] = result.SMS.Code
			payload["text"] = result.SMS.Text
		}
		return Response{Status: string(result.Status), Payload: payload}, nil
	default:
		return Response{Status: "unsupported", Payload: map[string]string{"kind": request.Kind}}, nil
	}
}
