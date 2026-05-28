package lifecycle

import "time"

type ConsumptionModel struct {
	PageType       PageType
	MinDwell       time.Duration
	MaxDwell       time.Duration
	ScrollRequired bool
	Interaction    string
}

func ModelForPageType(pageType PageType) ConsumptionModel {
	switch pageType {
	case PageArticle:
		return ConsumptionModel{PageType: pageType, MinDwell: 45 * time.Second, MaxDwell: 6 * time.Minute, ScrollRequired: true, Interaction: "read"}
	case PageSearchResults:
		return ConsumptionModel{PageType: pageType, MinDwell: 8 * time.Second, MaxDwell: 90 * time.Second, ScrollRequired: true, Interaction: "scan"}
	case PageListing:
		return ConsumptionModel{PageType: pageType, MinDwell: 20 * time.Second, MaxDwell: 4 * time.Minute, ScrollRequired: true, Interaction: "compare"}
	case PageProduct:
		return ConsumptionModel{PageType: pageType, MinDwell: 30 * time.Second, MaxDwell: 5 * time.Minute, ScrollRequired: true, Interaction: "inspect"}
	case PageForm:
		return ConsumptionModel{PageType: pageType, MinDwell: 20 * time.Second, MaxDwell: 3 * time.Minute, Interaction: "fill"}
	case PageAuth:
		return ConsumptionModel{PageType: pageType, MinDwell: 10 * time.Second, MaxDwell: 2 * time.Minute, Interaction: "authenticate"}
	case PageDashboard:
		return ConsumptionModel{PageType: pageType, MinDwell: 25 * time.Second, MaxDwell: 5 * time.Minute, ScrollRequired: true, Interaction: "monitor"}
	default:
		return ConsumptionModel{PageType: PageGeneric, MinDwell: 15 * time.Second, MaxDwell: 2 * time.Minute, Interaction: "browse"}
	}
}
