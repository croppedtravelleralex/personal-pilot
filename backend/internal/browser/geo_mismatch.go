package browser

import (
	"fmt"
	"personal-pilot/backend/internal/events"
	"personal-pilot/backend/internal/logger"
	"strings"
)

// timezoneToCountry maps IANA timezone identifiers to ISO 3166-1 alpha-2 country codes.
// This is a curated subset covering the timezones exposed in the fingerprint panel.
var timezoneToCountry = map[string]string{
	"Asia/Shanghai":       "CN",
	"Asia/Tokyo":          "JP",
	"Asia/Seoul":          "KR",
	"Asia/Singapore":      "SG",
	"Asia/Hong_Kong":      "HK",
	"Asia/Dubai":          "AE",
	"Asia/Kolkata":        "IN",
	"America/New_York":    "US",
	"America/Los_Angeles": "US",
	"America/Chicago":     "US",
	"America/Denver":      "US",
	"America/Toronto":     "CA",
	"America/Sao_Paulo":   "BR",
	"Europe/London":       "GB",
	"Europe/Paris":        "FR",
	"Europe/Berlin":       "DE",
	"Europe/Moscow":       "RU",
	"Australia/Sydney":    "AU",
	"Pacific/Auckland":    "NZ",
}

// GeoMismatchInfo holds information about a timezone-IP geolocation mismatch.
type GeoMismatchInfo struct {
	ProfileID     string `json:"profileId"`
	IPCountry     string `json:"ipCountry"`
	IPCity        string `json:"ipCity"`
	TimezoneIANA  string `json:"timezoneIana"`
	IsMismatch    bool   `json:"isMismatch"`
	MismatchScore int    `json:"mismatchScore"` // 0-100, higher = worse mismatch
}

// DetectGeoMismatch checks if the browser timezone matches the proxy IP's country.
// ipCountry should be an ISO 3166-1 alpha-2 country code (e.g. "US", "CN").
func DetectGeoMismatch(profileID, timezoneIANA, ipCountry, ipCity string, emitFn func(string, ...interface{})) {
	if timezoneIANA == "" || ipCountry == "" || emitFn == nil {
		return
	}

	log := logger.New("Browser")

	expectedCountry, ok := timezoneToCountry[timezoneIANA]
	if !ok {
		// Unknown timezone, skip check
		return
	}

	if strings.EqualFold(expectedCountry, ipCountry) {
		return // match
	}

	info := GeoMismatchInfo{
		ProfileID:     profileID,
		IPCountry:     ipCountry,
		IPCity:        ipCity,
		TimezoneIANA:  timezoneIANA,
		IsMismatch:    true,
		MismatchScore: crossCountryMismatchScore(timezoneIANA, ipCountry),
	}

	log.Warn("时区与代理 IP 地理位置不匹配",
		logger.F("profile_id", profileID),
		logger.F("timezone", timezoneIANA),
		logger.F("expected_country", expectedCountry),
		logger.F("actual_country", ipCountry),
		logger.F("mismatch_score", info.MismatchScore),
	)

	emitFn(events.EventRiskFingerprintTimezoneIP, map[string]interface{}{
		"profileId":     info.ProfileID,
		"ipCountry":     info.IPCountry,
		"ipCity":        info.IPCity,
		"timezoneIana":  info.TimezoneIANA,
		"isMismatch":    info.IsMismatch,
		"mismatchScore": info.MismatchScore,
	})
}

// crossCountryMismatchScore returns a 0-100 severity score for a timezone-country mismatch.
// Continent-level mismatches (e.g. Asia timezone + US IP) score higher.
func crossCountryMismatchScore(tzIANA, ipCountry string) int {
	tzCountry, ok := timezoneToCountry[tzIANA]
	if !ok {
		return 50
	}

	if strings.EqualFold(tzCountry, ipCountry) {
		return 0
	}

	// Same-continent pairs are less suspicious
	sameContinent := map[string][]string{
		"AS": {"CN", "JP", "KR", "SG", "HK", "AE", "IN"},
		"EU": {"GB", "FR", "DE", "RU"},
		"NA": {"US", "CA"},
		"SA": {"BR"},
		"OC": {"AU", "NZ"},
	}

	continentOf := func(code string) string {
		upper := strings.ToUpper(strings.TrimSpace(code))
		for continent, countries := range sameContinent {
			for _, c := range countries {
				if c == upper {
					return continent
				}
			}
		}
		return ""
	}

	tzContinent := continentOf(tzCountry)
	ipContinent := continentOf(ipCountry)

	if tzContinent != "" && tzContinent == ipContinent {
		return 30 // same continent, different country
	}

	return 80 // different continent
}

// timezoneFromFingerprintArgs extracts the timezone value from fingerprint CLI args.
func timezoneFromFingerprintArgs(args []string) string {
	for _, arg := range args {
		if strings.HasPrefix(arg, "--timezone=") {
			return strings.TrimPrefix(arg, "--timezone=")
		}
	}
	return ""
}

// CheckGeoMismatchFromProfile is a convenience function that extracts the timezone
// from a profile's fingerprint args and checks against the IP country.
func CheckGeoMismatchFromProfile(profileID string, fingerprintArgs []string, ipCountry, ipCity string, emitFn func(string, ...interface{})) {
	tz := timezoneFromFingerprintArgs(fingerprintArgs)
	if tz == "" {
		return
	}

	DetectGeoMismatch(profileID, tz, ipCountry, ipCity, emitFn)
}

// timezoneMapping is used to validate timezone values
var _ = fmt.Sprintf("%s", timezoneToCountry) // suppress unused import warning
