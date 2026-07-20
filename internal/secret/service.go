package secret

import (
	"context"

	"github.com/sachahjkl/dw/internal/contract"
)

type Service struct {
	store contract.SecretStore
}

func NewService(store contract.SecretStore) *Service {
	if store == nil {
		store = DefaultStore()
	}
	return &Service{store: store}
}

func (service *Service) Set(ctx context.Context, key contract.SecretKey, value contract.SecretValue) (SetReport, error) {
	return SetSecret(ctx, service.store, key, value)
}

func (service *Service) Get(ctx context.Context, key contract.SecretKey) (GetReport, error) {
	return GetSecret(ctx, service.store, key)
}

func (service *Service) Delete(ctx context.Context, key contract.SecretKey) (DeleteReport, error) {
	return DeleteSecret(ctx, service.store, key)
}

func (service *Service) List(ctx context.Context, root string) (ListReport, error) {
	return Discover(ctx, root, service.store)
}
