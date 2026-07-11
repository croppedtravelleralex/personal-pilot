package browser

import "strings"

// GeoLocale maps proxy country codes to coherent locale/timezone/UA hints.
type GeoLocale struct {
	Lang     string
	Accept   string
	Timezone string
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
