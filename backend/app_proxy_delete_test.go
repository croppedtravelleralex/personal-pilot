package backend

import (
	"strings"
	"testing"

	"personal-pilot/backend/internal/browser"
	"personal-pilot/backend/internal/config"
)

type proxyDeleteDAOStub struct {
	items   map[string]browser.Proxy
	deleted []string
}

func (s *proxyDeleteDAOStub) List() ([]browser.Proxy, error) {
	list := make([]browser.Proxy, 0, len(s.items))
	for _, item := range s.items {
		list = append(list, item)
	}
	return list, nil
}

func (s *proxyDeleteDAOStub) ListByGroup(string) ([]browser.Proxy, error) { return nil, nil }
func (s *proxyDeleteDAOStub) ListGroups() ([]string, error)               { return nil, nil }
func (s *proxyDeleteDAOStub) Upsert(proxy browser.Proxy) error {
	s.items[proxy.ProxyId] = proxy
	return nil
}
func (s *proxyDeleteDAOStub) Delete(proxyId string) error {
	delete(s.items, proxyId)
	s.deleted = append(s.deleted, proxyId)
	return nil
}
func (s *proxyDeleteDAOStub) DeleteAll() error {
	s.items = map[string]browser.Proxy{}
	return nil
}
func (s *proxyDeleteDAOStub) UpdateSpeedResult(string, bool, int64, string) error {
	return nil
}
func (s *proxyDeleteDAOStub) UpdateIPHealthResult(string, string) error { return nil }

func TestBrowserProxyDelete(t *testing.T) {
	t.Run("deletes normal proxy through dao", func(t *testing.T) {
		cfg := config.DefaultConfig()
		cfg.Browser.Proxies = []browser.Proxy{
			{ProxyId: "__direct__", ProxyName: "直连", ProxyConfig: "direct://"},
			{ProxyId: "p1", ProxyName: "代理 1", ProxyConfig: "http://127.0.0.1:30000"},
		}
		mgr := browser.NewManager(cfg, t.TempDir())
		dao := &proxyDeleteDAOStub{items: map[string]browser.Proxy{
			"__direct__": cfg.Browser.Proxies[0],
			"p1":         cfg.Browser.Proxies[1],
		}}
		mgr.ProxyDAO = dao
		app := &App{config: cfg, browserMgr: mgr, appRoot: t.TempDir()}

		if err := app.BrowserProxyDelete(" p1 "); err != nil {
			t.Fatalf("delete normal proxy: %v", err)
		}
		if _, ok := dao.items["p1"]; ok {
			t.Fatal("proxy p1 should be removed from dao")
		}
		if len(dao.deleted) != 1 || dao.deleted[0] != "p1" {
			t.Fatalf("unexpected deleted ids: %#v", dao.deleted)
		}
		for _, item := range app.config.Browser.Proxies {
			if item.ProxyId == "p1" {
				t.Fatal("proxy p1 should be removed from config snapshot")
			}
		}
	})

	t.Run("rejects builtin proxy", func(t *testing.T) {
		cfg := config.DefaultConfig()
		mgr := browser.NewManager(cfg, t.TempDir())
		dao := &proxyDeleteDAOStub{items: map[string]browser.Proxy{}}
		mgr.ProxyDAO = dao
		app := &App{config: cfg, browserMgr: mgr, appRoot: t.TempDir()}

		err := app.BrowserProxyDelete("__direct__")
		if err == nil || !strings.Contains(err.Error(), "内置代理不能删除") {
			t.Fatalf("expected builtin rejection, got %v", err)
		}
		if len(dao.deleted) != 0 {
			t.Fatalf("builtin proxy should not call dao delete: %#v", dao.deleted)
		}
	})

	t.Run("rejects empty id", func(t *testing.T) {
		app := &App{config: config.DefaultConfig()}
		err := app.BrowserProxyDelete(" ")
		if err == nil || !strings.Contains(err.Error(), "代理 ID 不能为空") {
			t.Fatalf("expected empty id rejection, got %v", err)
		}
	})
}
