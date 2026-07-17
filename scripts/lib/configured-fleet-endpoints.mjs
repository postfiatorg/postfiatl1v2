function requiredList(env, name) {
  const raw = String(env[name] || '').trim();
  if (!raw) throw new Error(`${name} must be supplied by the typed fleet configuration`);
  const values = raw.split(',').map(value => value.trim());
  if (values.some(value => !value)) throw new Error(`${name} contains an empty entry`);
  return values;
}

export function configuredFleetEndpoints(env = process.env) {
  const hosts = requiredList(env, 'VALIDATOR_HOSTS');
  const portValues = requiredList(env, 'VALIDATOR_RPC_PORTS');
  if (hosts.length !== portValues.length) {
    throw new Error('VALIDATOR_HOSTS and VALIDATOR_RPC_PORTS length mismatch');
  }
  const ports = portValues.map(value => Number(value));
  if (ports.some(port => !Number.isInteger(port) || port < 1 || port > 65535)) {
    throw new Error('VALIDATOR_RPC_PORTS contains an invalid port');
  }
  return { hosts, ports };
}
