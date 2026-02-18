# 틱 불균형 바 (Tick Imbalance Bars, TIBS)

## 개요

틱 불균형 바(TIBS)는 Marcos López de Prado가 *Advances in Financial Machine Learning* (2018)에서 제안한 정보 기반 바 샘플링 방법이다. 시간 바(고정 간격 샘플링)나 틱 바(N개 거래마다 샘플링)와 달리, TIBS는 **누적 틱 불균형**이 동적으로 조정되는 임계값을 초과할 때 바를 닫는다. 따라서 방향성 거래가 활발한 시기에는 바가 빠르게 형성되고, 균형 잡힌 시장에서는 느리게 형성된다.

## TIBS를 사용하는 이유

- **정보 기반 샘플링**: 임의의 시간이나 틱 수가 아닌, 의미 있는 시장 이벤트(방향성 압력)에 의해 바가 트리거된다.
- **우수한 통계적 특성**: TIBS로 샘플링된 수익률은 시간 바에 비해 IID(독립 동일 분포)에 더 가깝다.
- **적응적**: 임계값이 EWMA를 통해 시간에 따라 자동으로 조정되어, 변화하는 시장 미시구조에 적응한다.
- **불균형 감지**: 정보 거래, 모멘텀 전환, 방향성 확신을 감지하는 데 유용하다.

## 수학적 기초

### 틱 규칙 (Tick Rule)

각 거래는 매수 또는 매도로 분류된다:

```
b_t = +1  매수자 주도 거래
b_t = -1  매도자 주도 거래
```

Raven에서는 거래소(바이낸스)가 직접 제공하는 공격적 주문 방향(aggressor side)을 사용한다.

### 누적 불균형 (Theta)

각 바 내에서 누적 틱 불균형은:

```
θ_T = Σ(t=1..T) b_t
```

여기서 T는 현재 바에 누적된 틱 수이다.

### 바 닫기 조건

다음 조건이 충족되면 바가 닫힌다:

```
|θ_T| > |E[T] × (2·E[p_buy] - 1)|
```

여기서:
- `E[T]` = 바당 예상 틱 수 (과거 바 크기의 EWMA)
- `E[p_buy]` = 예상 매수 틱 비율 (과거 매수 비율의 EWMA)

임계값은 적응적이다: 최근 바에 더 많은 틱이 있었으면 임계값이 커지고(바가 커짐), 최근 불균형이 더 컸으면 임계값이 줄어든다(바가 빠르게 형성됨).

### EWMA 업데이트 (각 바 닫힘 후)

바가 `T_bar`개의 틱과 관찰된 `p_buy`로 닫힐 때:

```
E[T]     ← α_size × T_bar + (1 - α_size) × E[T]
E[p_buy] ← α_imbl × p_buy + (1 - α_imbl) × E[p_buy]
threshold ← E[T] × (2·E[p_buy] - 1)
```

`E[T]`는 `[size_min, size_max]`로 제한되어 비정상적인 바 크기를 방지한다.

## 하이퍼파라미터

| 파라미터 | 설정 키 | 기본값 | 설명 |
|---------|---------|--------|------|
| 초기 바 크기 | `initial_size` | 100.0 | 바당 예상 틱 수 `E[T]`의 시작 추정값 |
| 초기 매수 확률 | `initial_p_buy` | 0.7 | `E[p_buy]`의 시작 추정값 |
| 크기 EWMA 알파 | `alpha_size` | 0.1 | 바 크기 EWMA의 평활 계수 (높을수록 반응성 증가) |
| 불균형 EWMA 알파 | `alpha_imbl` | 0.1 | 매수 확률 EWMA의 평활 계수 |
| 크기 하한 | `size_min` 또는 `size_min_pct` | 초기값의 10% 아래 | 허용되는 최소 `E[T]` (바가 너무 작아지는 것을 방지) |
| 크기 상한 | `size_max` 또는 `size_max_pct` | 초기값의 10% 위 | 허용되는 최대 `E[T]` (바가 너무 커지는 것을 방지) |

### 크기 제한

제한은 두 가지 방식으로 지정할 수 있다:
- **백분율** (`size_min_pct`, `size_max_pct`): `initial_size` 기준 상대값. 예: `size_min_pct = 0.1`이면 `min = initial_size × 0.9`.
- **절대값** (`size_min`, `size_max`): 고정 틱 수.

둘 다 지정된 경우 백분율 방식이 우선한다.

## 출력

닫힌 각 바는 `Candle` protobuf 메시지로 방출된다:

| 필드 | 내용 |
|------|------|
| `symbol` | 거래 쌍 (예: BTCUSDT) |
| `timestamp` | 바 시작 시간 (에포크 이후 밀리초) |
| `open`, `high`, `low`, `close` | OHLC 가격 |
| `volume` | 바 내 총 거래량 |
| `interval` | 레이블 문자열 (예: `tib_small`, `tib_large`) |
| `buy_ticks` | 매수자 주도 거래 수 |
| `sell_ticks` | 매도자 주도 거래 수 |
| `total_ticks` | 바 내 총 거래 수 |
| `theta` | 최종 누적 틱 불균형 (부호 있는 정수) |

## 서비스 변형

Raven은 기본적으로 두 가지 TIBS 프로필을 실행한다:

- **tibs_small** (`port_tibs_small`): `initial_size = 100` — 자주 바를 생성, 작은 불균형에 민감
- **tibs_large** (`port_tibs_large`): `initial_size = 500` — 덜 자주 바를 생성, 더 큰 방향성 움직임을 포착

## 영속화

TIBS 바는 TimescaleDB의 `mart.bar__tick_imbalance` 테이블에 저장된다. `interval` 컬럼으로 small/large 프로필과 TIBS/TRBS를 구분한다.

## 참고 문헌

- López de Prado, M. (2018). *Advances in Financial Machine Learning*. Wiley. 2장: Financial Data Structures.
