package provider

type Request struct {
	Kind    string
	Payload map[string]string
}

type Response struct {
	Status  string
	Payload map[string]string
}

type Gateway interface {
	Call(request Request) (Response, error)
}
