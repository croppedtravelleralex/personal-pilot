package email

import (
	"crypto/rand"
	"math/big"
	"strings"
)

var firstNames = []string{
	"james", "mary", "robert", "patricia", "john", "jennifer",
	"michael", "linda", "david", "elizabeth", "william", "barbara",
	"richard", "susan", "joseph", "jessica", "thomas", "sarah",
	"christopher", "karen", "daniel", "lisa", "matthew", "nancy",
	"anthony", "betty", "mark", "margaret", "donald", "sandra",
	"steven", "ashley", "paul", "dorothy", "andrew", "kimberly",
	"joshua", "emily", "kenneth", "donna", "kevin", "michelle",
	"brian", "carol", "george", "amanda", "timothy", "melissa",
	"ronald", "deborah", "edward", "stephanie", "jason", "rebecca",
	"jeffrey", "sharon", "ryan", "laura", "jacob", "cynthia",
	"gary", "kathleen", "nicholas", "amy", "eric", "angela",
	"jonathan", "shirley", "stephen", "anna", "larry", "brenda",
	"justin", "pamela", "scott", "emma", "brandon", "nicole",
	"benjamin", "helen", "samuel", "samantha", "raymond", "katherine",
	"gregory", "christine", "frank", "debra", "alexander", "rachel",
	"patrick", "carolyn", "jack", "janet", "dennis", "catherine",
	"jerry", "maria", "tyler", "heather", "aaron", "diane",
}

var lastNames = []string{
	"smith", "johnson", "williams", "brown", "jones",
	"garcia", "miller", "davis", "rodriguez", "martinez",
	"hernandez", "lopez", "gonzalez", "wilson", "anderson",
	"thomas", "taylor", "moore", "jackson", "martin",
	"lee", "perez", "thompson", "white", "harris",
	"sanchen", "clark", "ramirez", "lewis", "robinson",
	"walker", "young", "allen", "king", "wright",
	"scott", "torres", "nguyen", "hill", "flores",
	"green", "adams", "nelson", "baker", "hall",
	"rivera", "campbell", "mitchell", "carter", "roberts",
	"gomez", "phillips", "evans", "turner", "diaz",
	"parker", "cruz", "edwards", "collins", "reyes",
	"stewart", "morris", "morales", "murphy", "cook",
	"rogers", "gutierrez", "ortiz", "morgan", "cooper",
	"peterson", "bailey", "reed", "kelly", "howard",
	"ramos", "kim", "cox", "ward", "richardson",
	"watson", "brooks", "chavez", "wood", "james",
	"bennett", "gray", "mendoza", "ruiz", "hughes",
	"price", "alvarez", "castillo", "sanders", "patel",
}

// GenerateHumanEmail creates a realistic human-like email address
// in the format: firstname.lastnameNN@domain
func GenerateHumanEmail(domain string) string {
	firstName := pickRandom(firstNames)
	lastName := pickRandom(lastNames)
	n, _ := rand.Int(rand.Reader, big.NewInt(50))
	suffix := int(n.Int64()) + 50 // 50-99 like birth years
	return strings.ToLower(firstName + "." + lastName + itoa(suffix) + "@" + domain)
}

// GenerateHumanPassword creates a memorable password pattern like "Word1234!"
func GenerateHumanPassword() string {
	word := pickRandom([]string{
		"Summer", "Winter", "Spring", "Autumn", "Coffee", "Dragon",
		"Tiger", "Eagle", "River", "Mountain", "Forest", "Ocean",
		"Silver", "Golden", "Crystal", "Thunder", "Shadow", "Phoenix",
		"Jupiter", "Mercury", "Comet", "Galaxy", "Horizon", "Sunrise",
	})
	n1, _ := rand.Int(rand.Reader, big.NewInt(9000))
	num := 1000 + int(n1.Int64())
	return word + itoa(num) + "!"
}

// ─── helpers ──────────────────────────────────────────────────────────────────

func pickRandom(list []string) string {
	n, err := rand.Int(rand.Reader, big.NewInt(int64(len(list))))
	if err != nil {
		return list[0]
	}
	return list[n.Int64()]
}

func itoa(n int) string {
	if n == 0 {
		return "0"
	}
	digits := make([]byte, 0, 8)
	for n > 0 {
		digits = append([]byte{byte('0' + n%10)}, digits...)
		n /= 10
	}
	return string(digits)
}

// SecureHex returns a cryptographically random hex string of given length.
func SecureHex(n int) string {
	const hexChars = "0123456789abcdef"
	if n <= 0 {
		return ""
	}
	needed := (n + 1) / 2
	randBytes := make([]byte, needed)
	_, _ = rand.Read(randBytes)

	b := make([]byte, n)
	for i := range b {
		idx := i / 2
		if i%2 == 0 {
			b[i] = hexChars[randBytes[idx]>>4]
		} else {
			b[i] = hexChars[randBytes[idx]&0x0f]
		}
	}
	return string(b)
}
