import os
import pandas as pd
from typing import Optional, Dict, Any, List
from influxdb_client import InfluxDBClient
from influxdb_client.client.query_api import QueryApi


class InfluxDBHandler:
    def __init__(self) -> None:
        self.url: str = os.getenv("INFLUX_URL", "http://influxdb:8086")
        self.token: str = os.getenv("INFLUX_TOKEN", "raven_master_token_for_the_iron_bank")
        self.org: str = os.getenv("INFLUX_ORG", "house_raven")
        self.bucket: str = os.getenv("INFLUX_BUCKET", "market_data")
        self._client: Optional[InfluxDBClient] = None

    @property
    def client(self) -> InfluxDBClient:
        if self._client is None:
            self._client = InfluxDBClient(
                url=self.url,
                token=self.token,
                org=self.org
            )
        return self._client

    def query_market_data(self, time_range: str = "1h", symbol: Optional[str] = None) -> pd.DataFrame:
        """Query market data from InfluxDB"""
        query_api: QueryApi = self.client.query_api()

        symbol_filter = f'|> filter(fn: (r) => r.symbol == "{symbol}")' if symbol else ''

        query = f'''
        from(bucket: "{self.bucket}")
            |> range(start: -{time_range})
            |> filter(fn: (r) => r._measurement == "market_data")
            {symbol_filter}
            |> pivot(rowKey:["_time"], columnKey: ["_field"], valueColumn: "_value")
        '''

        try:
            result = query_api.query(query, org=self.org)

            data: List[Dict[str, Any]] = []
            for table in result:
                for record in table.records:
                    data.append({
                        'time': record.get_time(),
                        'symbol': record.values.get('symbol', ''),
                        'price': record.values.get('price', 0),
                        'volume': record.values.get('volume', 0),
                        'bid': record.values.get('bid', 0),
                        'ask': record.values.get('ask', 0),
                        'high': record.values.get('high', 0),
                        'low': record.values.get('low', 0),
                        'open': record.values.get('open', 0),
                        'close': record.values.get('close', 0)
                    })

            return pd.DataFrame(data)
        except Exception as e:
            print(f"Error querying market data: {str(e)}")
            return pd.DataFrame()

    def query_system_metrics(self) -> Dict[str, Any]:
        """Get system health metrics"""
        query_api: QueryApi = self.client.query_api()

        query = f'''
        from(bucket: "{self.bucket}")
            |> range(start: -5m)
            |> filter(fn: (r) => r._measurement == "system_metrics")
            |> last()
        '''

        try:
            result = query_api.query(query, org=self.org)
            metrics: Dict[str, Any] = {}

            for table in result:
                for record in table.records:
                    metrics[record.get_field()] = record.get_value()

            return metrics
        except Exception as e:
            print(f"Error querying system metrics: {str(e)}")
            return {}

    def get_available_symbols(self) -> List[str]:
        """Get list of available symbols"""
        query_api: QueryApi = self.client.query_api()

        query = f'''
        from(bucket: "{self.bucket}")
            |> range(start: -24h)
            |> filter(fn: (r) => r._measurement == "market_data")
            |> distinct(column: "symbol")
            |> keep(columns: ["_value"])
        '''

        try:
            result = query_api.query(query, org=self.org)
            symbols: List[str] = []

            for table in result:
                for record in table.records:
                    if record.get_value():
                        symbols.append(record.get_value())

            return sorted(list(set(symbols)))
        except Exception as e:
            print(f"Error getting symbols: {str(e)}")
            return []

    def query_custom(self, query: str) -> pd.DataFrame:
        """Execute custom Flux query"""
        query_api: QueryApi = self.client.query_api()

        try:
            result = query_api.query(query, org=self.org)

            data: List[Dict[str, Any]] = []
            for table in result:
                for record in table.records:
                    row_data: Dict[str, Any] = {
                        'time': record.get_time(),
                        'measurement': record.get_measurement(),
                        'field': record.get_field(),
                        'value': record.get_value()
                    }
                    # Add all tags
                    for key, value in record.values.items():
                        if key not in ['_time', '_measurement', '_field', '_value', 'result', 'table']:
                            row_data[key] = value

                    data.append(row_data)

            return pd.DataFrame(data)
        except Exception as e:
            print(f"Error executing custom query: {str(e)}")
            return pd.DataFrame()

    def close(self) -> None:
        """Close the client connection"""
        if self._client:
            self._client.close()
            self._client = None