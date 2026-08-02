package browser

import "strings"

// GeoLocale maps proxy country codes to coherent locale/timezone/UA hints.
type GeoLocale struct {
	Lang     string
	Accept   string
	Timezone string
}

// GeoCoordinate is a city-level lat/lon used for Emulation.setGeolocationOverride.
type GeoCoordinate struct {
	Lat      float64
	Lon      float64
	Accuracy float64
}

var geoLocaleTable = map[string]GeoLocale{
	"US": {Lang: "en-US", Accept: "en-US,en;q=0.9", Timezone: "America/New_York"},
	"CA": {Lang: "en-CA", Accept: "en-CA,en;q=0.9", Timezone: "America/Toronto"},
	"GB": {Lang: "en-GB", Accept: "en-GB,en;q=0.9", Timezone: "Europe/London"},
	"UK": {Lang: "en-GB", Accept: "en-GB,en;q=0.9", Timezone: "Europe/London"},
	"DE": {Lang: "de-DE", Accept: "de-DE,de;q=0.9,en;q=0.8", Timezone: "Europe/Berlin"},
	"FR": {Lang: "fr-FR", Accept: "fr-FR,fr;q=0.9,en;q=0.8", Timezone: "Europe/Paris"},
	"JP": {Lang: "ja-JP", Accept: "ja-JP,ja;q=0.9,en;q=0.8", Timezone: "Asia/Tokyo"},
	"CN": {Lang: "zh-CN", Accept: "zh-CN,zh;q=0.9,en;q=0.8", Timezone: "Asia/Shanghai"},
	"AU": {Lang: "en-AU", Accept: "en-AU,en;q=0.9", Timezone: "Australia/Sydney"},
}

// City coordinates are public city-center references (not spoofed street addresses).
var geoCoordinateTable = map[string]GeoCoordinate{
	"US|NEW YORK":    {40.7128, -74.0060, 100},
	"US|LOS ANGELES": {34.0522, -118.2437, 100},
	"US|":            {40.7128, -74.0060, 5000},
	"CA|TORONTO":     {43.6532, -79.3832, 100},
	"CA|":            {43.6532, -79.3832, 5000},
	"GB|LONDON":      {51.5074, -0.1278, 100},
	"UK|LONDON":      {51.5074, -0.1278, 100},
	"GB|":            {51.5074, -0.1278, 5000},
	"DE|BERLIN":      {52.5200, 13.4050, 100},
	"DE|":            {52.5200, 13.4050, 5000},
	"FR|PARIS":       {48.8566, 2.3522, 100},
	"FR|":            {48.8566, 2.3522, 5000},
	"JP|TOKYO":       {35.6762, 139.6503, 100},
	"JP|":            {35.6762, 139.6503, 5000},
	"CN|SHANGHAI":    {31.2304, 121.4737, 100},
	"CN|BEIJING":     {39.9042, 116.4074, 100},
	"CN|":            {31.2304, 121.4737, 5000},
	"AU|SYDNEY":      {-33.8688, 151.2093, 100},
	"AU|":            {-33.8688, 151.2093, 5000},
}

// LookupGeoCoordinate resolves country/city to a coordinate; falls back to country default.
func LookupGeoCoordinate(country, city string) (GeoCoordinate, bool) {
	country = strings.ToUpper(strings.TrimSpace(country))
	city = strings.ToUpper(strings.TrimSpace(city))
	if country == "" {
		return GeoCoordinate{}, false
	}
	if c, ok := geoCoordinateTable[country+"|"+city]; ok {
		return c, true
	}
	if c, ok := geoCoordinateTable[country+"|"]; ok {
		return c, true
	}
	return GeoCoordinate{}, false
}

// ApplyGeoLocale overrides locale/timezone args to match proxy geography (anti timezone drift).
func ApplyGeoLocale(country string, fingerprintArgs, launchArgs []string) ([]string, []string) {
	country = strings.ToUpper(strings.TrimSpace(country))
	loc, ok := geoLocaleTable[country]
	if !ok {
		return fingerprintArgs, launchArgs
	}
	fingerprintArgs = stripLaunchArgPrefixes(fingerprintArgs, "--lang", "--accept-lang", "--timezone")
	launchArgs = stripLaunchArgPrefixes(launchArgs, "--lang", "--accept-lang", "--timezone")
	fingerprintArgs = append(fingerprintArgs,
		"--lang="+loc.Lang,
		"--accept-lang="+loc.Accept,
		"--timezone="+loc.Timezone,
	)
	return fingerprintArgs, launchArgs
}

// ForceWindowsDesktopIdentity rewrites platform/UA tokens to a Windows desktop profile.
// Used when a foreign-exit proxy is bound but persona assignment randomly picked macOS.
func ForceWindowsDesktopIdentity(fingerprintArgs, launchArgs []string) ([]string, []string) {
	fingerprintArgs = stripLaunchArgPrefixes(fingerprintArgs,
		"--fingerprint-platform",
		"--fingerprint-platform-version",
		"--user-agent",
	)
	launchArgs = stripLaunchArgPrefixes(launchArgs,
		"--fingerprint-platform",
		"--fingerprint-platform-version",
		"--user-agent",
	)

	// Preserve brand-version if present so UA major stays coherent with core.
	brandVersion := ""
	for _, arg := range append(append([]string{}, fingerprintArgs...), launchArgs...) {
		if strings.HasPrefix(arg, "--fingerprint-brand-version=") {
			brandVersion = strings.TrimPrefix(arg, "--fingerprint-brand-version=")
			break
		}
	}
	if brandVersion == "" {
		brandVersion = DefaultChromiumVersion
	}
	ua := "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/" + brandVersion + " Safari/537.36"
	fingerprintArgs = append(fingerprintArgs,
		"--fingerprint-platform=windows",
		"--fingerprint-platform-version=10.0.0",
		"--user-agent="+ua,
	)
	return fingerprintArgs, launchArgs
}
