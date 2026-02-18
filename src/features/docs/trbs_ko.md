# 틱 런 바 (Tick Run Bars, TRBS)

## 개요

틱 런 바(TRBS)는 López de Prado (2018)의 또 다른 정보 기반 바 샘플링 방법이다. TIBS가 누적 *불균형*(순매수 - 순매도)을 측정하는 반면, TRBS는 가장 긴 *런(run)* — 바 내에서 같은 방향의 연속 틱 최대 시퀀스를 측정한다. 가장 긴 런이 동적으로 조정되는 임계값을 초과하면 바가 닫히며, 이는 한 방향으로의 비정상적인 연속 주문 흐름을 나타낸다.

## TRBS를 사용하는 이유

- **연속 주문 흐름 감지**: 많은 연속 거래가 같은 방향으로 진행되는 패턴을 포착하며, 이는 종종 알고리즘 또는 기관 매매를 시사한다.
- **TIBS와 상호 보완적**: TIBS는 순 불균형을 감지하고, TRBS는 방향의 지속성을 감지한다. 시장이 순매수/매도가 균형 잡혀 있어도 의심스러운 런을 보일 수 있다.
- **적응적 임계값**: TIBS와 마찬가지로 임계값이 EWMA를 통해 시간에 따라 조정되어 현재 시장 레짐에 적응한다.

## 수학적 기초

### 틱 방향

각 거래를 분류한다:

```
b_t = Buy   매수자 주도 거래
b_t = Sell   매도자 주도 거래
```

### 현재 런 추적

"런"은 같은 방향의 연속 틱 시퀀스이다. 각 틱에서:

```
if b_t == run_sign:
    current_run += 1
else:
    θ = max(θ, current_run)    // 완료된 런 길이 기록
    run_sign = b_t              // 새 런 시작
    current_run = 1
```

### 세타 (최장 런)

`θ`는 현재 바 내에서 관찰된 가장 긴 런이다:

```
θ = max(이 바의 모든 완료된 런 길이)
```

### 바 닫기 조건

다음 조건이 충족되면 바가 닫힌다:

```
θ > E[T] × max(E[p_buy], 1 - E[p_buy])
```

여기서:
- `E[T]` = 바당 예상 틱 수 (과거 바 크기의 EWMA)
- `E[p_buy]` = 예상 매수 확률 (관찰된 매수 비율의 EWMA)
- `max(p, 1-p)` = 우세한 방향의 확률

직관: 균형 잡힌 시장(`p_buy ≈ 0.5`)에서 T개 틱의 예상 최장 런은 T와 함께 증가한다. 임계값은 바 크기와 방향 편향 모두에 비례한다.

### EWMA 업데이트 (각 바 닫힘 후)

```
E[p_buy] ← α_prob × p_buy_관찰값 + (1 - α_prob) × E[p_buy]
E[T]     ← α_size × T_bar + (1 - α_size) × E[T]
E[T]     = clamp(E[T], size_min, size_max)
threshold ← E[T] × max(E[p_buy], 1 - E[p_buy])
```

## 하이퍼파라미터

TRBS는 편의를 위해 동일한 `TibsConfig` 구조체를 재사용한다. `alpha_imbl` 파라미터는 확률 EWMA 알파로 사용된다.

| 파라미터 | 설정 키 | 기본값 | 설명 |
|---------|---------|--------|------|
| 초기 바 크기 | `initial_size` | 100.0 | `E[T]`의 시작 추정값 |
| 초기 매수 확률 | `initial_p_buy` | 0.7 | `E[p_buy]`의 시작 추정값 |
| 크기 EWMA 알파 | `alpha_size` | 0.1 | 바 크기의 평활 계수 |
| 확률 EWMA 알파 | `alpha_imbl` | 0.1 | 매수 확률의 평활 계수 (TIBS 설정 키 재사용) |
| 크기 제한 | `size_min_pct` / `size_max_pct` | 초기값의 ±10% | `E[T]`의 제한 범위 |

## 출력

닫힌 각 바는 `Candle` protobuf 메시지로 방출된다:

| 필드 | 내용 |
|------|------|
| `symbol` | 거래 쌍 |
| `timestamp` | 바 시작 시간 (밀리초) |
| `open`, `high`, `low`, `close` | OHLC 가격 |
| `volume` | 총 거래량 |
| `interval` | 레이블 (예: `trb_small`, `trb_large`) |
| `buy_ticks` | 매수자 주도 거래 수 |
| `sell_ticks` | 매도자 주도 거래 수 |
| `total_ticks` | 총 거래 수 |
| `theta` | 최장 런 길이 (부호 없는 정수) |

## 서비스 변형

- **trbs_small** (`port_trbs_small`): `initial_size = 100`
- **trbs_large** (`port_trbs_large`): `initial_size = 500`

## 영속화

TRBS 바는 TimescaleDB의 `mart.bar__tick_imbalance` 테이블에 저장된다 (TIBS와 공유). `interval` 컬럼 (예: `trb_small` vs `tib_small`)으로 바 유형을 구분한다.

## TIBS와의 차이점

| 관점 | TIBS | TRBS |
|------|------|------|
| 측정 대상 | 순 불균형 (매수 - 매도) | 최장 연속 런 |
| 닫기 조건 | \|Σ b_t\| > 임계값 | max_run > 임계값 |
| 민감도 | 총 방향성 압력 | 연속적 주문 흐름 |
| 사용 사례 | 전체적 방향 편향 | 알고리즘/모멘텀 감지 |

## 참고 문헌

- López de Prado, M. (2018). *Advances in Financial Machine Learning*. Wiley. 2장: Financial Data Structures.
