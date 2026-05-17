package proxy

import (
	"encoding/json"
	"fmt"
	"strings"

	"personal-pilot/backend/internal/config"
)

// PriorityTier defines proxy quality tiers.
type PriorityTier int

const (
	Tier1TaiwanResidential PriorityTier = 1
	Tier2LowLatency        PriorityTier = 2
	Tier3EuropeBackup      PriorityTier = 3
	Tier4AnyNonCN          PriorityTier = 4
)

// IPHealthData holds parsed fields from LastIPHealthJSON.
type IPHealthData struct {
	Country        string  `json:"country"`
	CountryCode    string  `json:"countryCode"`
	Region         string  `json:"region"`
	City           string  `json:"city"`
	FraudScore     float64 `json:"fraudScore"`
	IsResidential  bool    `json:"isResidential"`
	IsBroadcast    bool    `json:"isBroadcast"`
	ASOrganization string  `json:"asOrganization"`
}

// ProxyDAO is the minimal interface needed by ProxySelector.
type ProxyDAO interface {
	List() ([]config.BrowserProxy, error)
}

// ProxySelector selects the best proxy based on tier criteria.
type ProxySelector struct {
	dao       ProxyDAO
	blacklist map[string]bool
	usedIDs   map[string]bool
	order     []string
}

// NewProxySelector creates a new ProxySelector.
func NewProxySelector(dao ProxyDAO) *ProxySelector {
	return &ProxySelector{
		dao:       dao,
		blacklist: make(map[string]bool),
		usedIDs:   make(map[string]bool),
	}
}

// SelectBest selects the best available proxy for the given tier.
func (s *ProxySelector) SelectBest(tier PriorityTier) (*config.BrowserProxy, error) {
	all, err := s.dao.List()
	if err != nil {
		return nil, fmt.Errorf("proxy list: %w", err)
	}

	candidates := s.filterByTier(all, tier)
	if len(candidates) == 0 {
		// Auto-degrade tier
		for t := tier + 1; t <= Tier4AnyNonCN; t++ {
			candidates = s.filterByTier(all, t)
			if len(candidates) > 0 {
				break
			}
		}
	}

	if len(candidates) == 0 {
		return nil, fmt.Errorf("no available proxy for tier %d", tier)
	}

	// Return best (lowest fraud score first)
	return &candidates[0], nil
}

// SelectNextBest selects another proxy in the same tier, auto-degrading if needed.
func (s *ProxySelector) SelectNextBest(previous *config.BrowserProxy) (*config.BrowserProxy, error) {
	if previous != nil {
		s.Blacklist(previous.ProxyId)
	}
	return s.SelectBest(Tier1TaiwanResidential) // will auto-degrade
}

// Blacklist marks a proxy as permanently unusable.
func (s *ProxySelector) Blacklist(id string) {
	s.blacklist[id] = true
}

// MarkUsed records a proxy as used (prevent reuse).
func (s *ProxySelector) MarkUsed(id string) {
	s.usedIDs[id] = true
}

// UsedIDs returns all used proxy IDs.
func (s *ProxySelector) UsedIDs() []string {
	ids := make([]string, 0, len(s.usedIDs))
	for id := range s.usedIDs {
		ids = append(ids, id)
	}
	return ids
}

// BlacklistedIDs returns all blacklisted proxy IDs.
func (s *ProxySelector) BlacklistedIDs() []string {
	ids := make([]string, 0, len(s.blacklist))
	for id := range s.blacklist {
		ids = append(ids, id)
	}
	return ids
}

func (s *ProxySelector) filterByTier(all []config.BrowserProxy, tier PriorityTier) []config.BrowserProxy {
	var result []config.BrowserProxy

	for _, p := range all {
		if s.blacklist[p.ProxyId] {
			continue
		}
		if s.usedIDs[p.ProxyId] {
			continue
		}

		health := parseIPHealth(p.LastIPHealthJSON)
		if !matchesTier(health, p, tier) {
			continue
		}

		result = append(result, p)
	}

	// Fallback: if pool exhausted (all used), clear used set and retry.
	// This allows proxy reuse when there aren't enough fresh IPs for the batch.
	if len(result) == 0 {
		s.usedIDs = make(map[string]bool)
		for _, p := range all {
			if s.blacklist[p.ProxyId] {
				continue
			}
			health := parseIPHealth(p.LastIPHealthJSON)
			if !matchesTier(health, p, tier) {
				continue
			}
			result = append(result, p)
		}
	}

	// Sort by fraud score ascending
	sortByFraudScore(result)
	return result
}

func matchesTier(health *IPHealthData, p config.BrowserProxy, tier PriorityTier) bool {
	country := strings.ToUpper(health.CountryCode)
	if country == "" {
		country = strings.ToUpper(health.Country)
	}

	isRes := health.IsResidential
	fs := health.FraudScore
	latency := p.LastLatencyMs

	switch tier {
	case Tier1TaiwanResidential:
		return country == "TW" && isRes && fs < 15
	case Tier2LowLatency:
		if !contains([]string{"US", "JP", "KR", "SG"}, country) {
			return false
		}
		if fs >= 30 {
			return false
		}
		if latency > 0 && latency >= 1000 {
			return false
		}
		return true
	case Tier3EuropeBackup:
		if !contains([]string{"GB", "FR", "DE", "NL", "CA", "AU"}, country) {
			return false
		}
		if fs >= 40 {
			return false
		}
		if latency > 0 && latency >= 2000 {
			return false
		}
		return true
	case Tier4AnyNonCN:
		if country == "CN" {
			return false
		}
		if fs >= 50 {
			return false
		}
		return true
	default:
		return false
	}
}

// ParseIPHealthJSON is the exported wrapper for IP health JSON parsing.
func ParseIPHealthJSON(jsonStr string) *IPHealthData {
	return parseIPHealth(jsonStr)
}

func parseIPHealth(jsonStr string) *IPHealthData {
	if jsonStr == "" || jsonStr == "{}" {
		return &IPHealthData{}
	}
	var data IPHealthData
	if err := json.Unmarshal([]byte(jsonStr), &data); err != nil {
		// Try nested structure from ippure/ip-api
		var wrapper map[string]interface{}
		if err2 := json.Unmarshal([]byte(jsonStr), &wrapper); err2 != nil {
			return &IPHealthData{}
		}
		// Extract common fields
		if v, ok := wrapper["country"]; ok {
			data.Country = fmt.Sprint(v)
		}
		if v, ok := wrapper["countryCode"]; ok {
			data.CountryCode = fmt.Sprint(v)
		} else if v, ok := wrapper["country_code"]; ok {
			data.CountryCode = fmt.Sprint(v)
		}
		if v, ok := wrapper["regionName"]; ok {
			data.Region = fmt.Sprint(v)
		}
		if v, ok := wrapper["city"]; ok {
			data.City = fmt.Sprint(v)
		}
		// Fraud score may come from trust_score
		if v, ok := wrapper["fraudScore"]; ok {
			data.FraudScore = toFloat64(v)
		} else if v, ok := wrapper["fraud_score"]; ok {
			data.FraudScore = toFloat64(v)
		}
		if v, ok := wrapper["hosting"]; ok {
			data.IsResidential = !isTrueish(v)
		}
	}
	return &data
}

func toFloat64(v interface{}) float64 {
	switch n := v.(type) {
	case float64:
		return n
	case float32:
		return float64(n)
	case int:
		return float64(n)
	case int64:
		return float64(n)
	case json.Number:
		f, _ := n.Float64()
		return f
	default:
		return 0
	}
}

func isTrueish(v interface{}) bool {
	switch n := v.(type) {
	case bool:
		return n
	case string:
		return strings.EqualFold(n, "true") || n == "1"
	case float64:
		return n != 0
	default:
		return false
	}
}

func contains(list []string, item string) bool {
	for _, s := range list {
		if s == item {
			return true
		}
	}
	return false
}

func sortByFraudScore(proxies []config.BrowserProxy) {
	for i := 0; i < len(proxies); i++ {
		for j := i + 1; j < len(proxies); j++ {
			hi := parseIPHealth(proxies[i].LastIPHealthJSON)
			hj := parseIPHealth(proxies[j].LastIPHealthJSON)
			if hi.FraudScore > hj.FraudScore {
				proxies[i], proxies[j] = proxies[j], proxies[i]
			}
		}
	}
}
