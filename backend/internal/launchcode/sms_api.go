package launchcode

import (
	"net/http"
	"strings"
	"time"

	"personal-pilot/backend/internal/logger"
	"personal-pilot/backend/internal/sms"
)

type buyNumberRequest struct {
	Service  string  `json:"service"`
	Country  string  `json:"country"`
	Operator string  `json:"operator"`
	MaxPrice float64 `json:"maxPrice"`
}

func (s *LaunchServer) SetSmsManager(m *sms.Manager) {
	s.smsManager = m
}

func (s *LaunchServer) HandleSmsBuyNumber(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if s.smsManager == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "sms service not available"})
		return
	}

	var req buyNumberRequest
	if status, errMsg := decodeLimitedJSONBody(r, &req); errMsg != "" {
		writeJSON(w, status, map[string]interface{}{"ok": false, "error": errMsg})
		return
	}
	if req.Service == "" || req.Country == "" {
		writeJSON(w, http.StatusBadRequest, map[string]interface{}{"ok": false, "error": "service and country are required"})
		return
	}

	buyReq := &sms.BuyRequest{
		Service:  req.Service,
		Country:  req.Country,
		Operator: req.Operator,
		MaxPrice: req.MaxPrice,
	}

	number, err := s.smsManager.AcquireNumber(r.Context(), buyReq)
	if err != nil {
		logger.New("SmsAPI").Error("buy number failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": "sms number purchase failed"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"id":        number.ID,
			"phone":     number.Phone,
			"price":     number.Price,
			"status":    string(number.Status),
			"country":   number.Country,
			"service":   number.Service,
			"expiresAt": number.ExpiresAt.Format(time.RFC3339),
		},
	})
}

func (s *LaunchServer) handleSmsByID(w http.ResponseWriter, r *http.Request) {
	if s.smsManager == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "sms service not available"})
		return
	}

	path := strings.TrimPrefix(r.URL.Path, "/api/sms/number/")
	path = strings.TrimSpace(path)

	if strings.HasSuffix(path, "/status") {
		orderID := strings.TrimSuffix(path, "/status")
		orderID = strings.TrimSpace(orderID)
		if orderID == "" || strings.Contains(orderID, "/") {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "order not found"})
			return
		}
		if r.Method != http.MethodGet {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		s.handleSmsStatus(w, r, orderID)
		return
	}

	if strings.HasSuffix(path, "/cancel") {
		orderID := strings.TrimSuffix(path, "/cancel")
		orderID = strings.TrimSpace(orderID)
		if orderID == "" || strings.Contains(orderID, "/") {
			writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "order not found"})
			return
		}
		if r.Method != http.MethodPost {
			writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
			return
		}
		s.handleSmsCancel(w, r, orderID)
		return
	}

	writeJSON(w, http.StatusNotFound, map[string]interface{}{"ok": false, "error": "not found"})
}

func (s *LaunchServer) handleSmsStatus(w http.ResponseWriter, r *http.Request, orderID string) {
	result, err := s.smsManager.CheckSMS(r.Context(), orderID)
	if err != nil {
		logger.New("SmsAPI").Error("check sms failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": "sms status check failed"})
		return
	}

	data := map[string]interface{}{
		"id":     orderID,
		"status": string(result.Status),
	}
	if result.SMS != nil {
		data["sms"] = map[string]interface{}{
			"code":       result.SMS.Code,
			"text":       result.SMS.Text,
			"sender":     result.SMS.Sender,
			"receivedAt": result.SMS.ReceivedAt.Format(time.RFC3339),
		}
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok":   true,
		"data": data,
	})
}

func (s *LaunchServer) handleSmsCancel(w http.ResponseWriter, r *http.Request, orderID string) {
	if err := s.smsManager.CancelByID(r.Context(), orderID); err != nil {
		logger.New("SmsAPI").Error("cancel sms failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": "sms cancellation failed"})
		return
	}
	writeJSON(w, http.StatusOK, map[string]interface{}{"ok": true, "canceled": true})
}

func (s *LaunchServer) HandleSmsBalance(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		writeJSON(w, http.StatusMethodNotAllowed, map[string]interface{}{"ok": false, "error": "method not allowed"})
		return
	}
	if s.smsManager == nil {
		writeJSON(w, http.StatusServiceUnavailable, map[string]interface{}{"ok": false, "error": "sms service not available"})
		return
	}

	balance, err := s.smsManager.GetBalance(r.Context())
	if err != nil {
		logger.New("SmsAPI").Error("get balance failed", logger.F("error", err.Error()))
		writeJSON(w, http.StatusInternalServerError, map[string]interface{}{"ok": false, "error": "sms balance check failed"})
		return
	}

	providerBalances := make(map[string]float64)
	for _, p := range s.smsManager.Providers() {
		bal, err := p.GetBalance(r.Context())
		if err == nil {
			providerBalances[p.Name()] = bal
		}
	}

	writeJSON(w, http.StatusOK, map[string]interface{}{
		"ok": true,
		"data": map[string]interface{}{
			"balance":    balance,
			"byProvider": providerBalances,
		},
	})
}
