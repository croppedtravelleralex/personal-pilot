package discovery

import "testing"

func TestDiscoveryContracts(t *testing.T) {
	if ClassifyPage("https://x/login", "", nil) != PageKindAuth {
		t.Fatal("auth classify failed")
	}
	if InferFieldKind("user_email") != "email" {
		t.Fatal("email field failed")
	}
}

func TestDiscoveryFlowAndTemplate(t *testing.T) {
	flow := InferFlow("https://x/register", "Register", `<div class="g-recaptcha"></div>`, []Field{{Name: "email", Kind: InferFieldKind("email"), Selector: "#email"}})
	if flow.PageKind != PageKindAuth || len(flow.Forms) != 1 || len(flow.Challenges) != 1 {
		t.Fatalf("flow = %+v", flow)
	}
	wf := FlowToWorkflowTemplate("wf-discovered", flow)
	if len(wf.Steps) < 3 {
		t.Fatalf("workflow = %+v", wf)
	}
}
