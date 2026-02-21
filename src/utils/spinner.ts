import ora, { Ora } from 'ora';

const spinnerInstance: Ora = ora();

export const spinner = {
  start(text: string): void {
    spinnerInstance.start(text);
  },

  succeed(text: string): void {
    spinnerInstance.succeed(text);
  },

  fail(text: string): void {
    spinnerInstance.fail(text);
  },

  stop(): void {
    spinnerInstance.stop();
  },

  isSpinning(): boolean {
    return spinnerInstance.isSpinning;
  },
};
